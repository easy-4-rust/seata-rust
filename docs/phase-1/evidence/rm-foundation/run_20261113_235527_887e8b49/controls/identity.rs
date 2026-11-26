use super::branch_lease::LeaseGuards;
use crate::{
    BranchIdentity, BranchKey, BranchLease, BranchPhase, BranchRecordState, JournalOptions, RmError,
};
use seata_protocol::transaction::{BranchCommitRequest, BranchRollbackRequest};
use seata_protocol::{Message, PayloadCodec};
use seata_transport::ResourceKey;
use std::collections::HashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use tokio::sync::Semaphore;

const HEADER: &[u8; 8] = b"SRMJNL01";
const RECORD_OVERHEAD: usize = 4 + 8 + 1 + 4;

/// 版本化、有界、独占的分支追加日志；克隆共享文件及分支租约，不复制文件句柄。
/// 对应 Java: RM二阶段幂等/恢复职责；Rust本地持久协调扩展，不能替代数据库Undo事务。
#[derive(Clone)]
pub struct FileBranchStore {
    inner: Arc<StoreInner>,
}
// 所有克隆共享同一文件锁和分支锁表，避免不同Dispatcher各自串行却互相并发。
struct StoreInner {
    options: JournalOptions,
    journal: Mutex<Journal>,
    key_locks: Mutex<HashMap<BranchKey, Weak<Semaphore>>>,
    lease_budget: Arc<Semaphore>,
}
struct Journal {
    file: File,
    entries: HashMap<BranchKey, StoredRecord>,
    bytes: u64,
    sequence: u64,
    faulted: bool,
}
#[derive(Clone)]
struct StoredRecord {
    identity: BranchIdentity,
    phase: BranchPhase,
    state: BranchRecordState,
}

impl fmt::Debug for FileBranchStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileBranchStore")
            .field("options", &self.inner.options)
            .finish_non_exhaustive()
    }
}

impl FileBranchStore {
    /// 参数为显式文件路径及预算；取得独占锁后严格重放，损坏或超限原样拒绝。
    /// 文件父目录必须已存在；不从任何事务字段派生文件名。
    pub async fn open(path: PathBuf, options: JournalOptions) -> Result<Self, RmError> {
        let journal = blocking(move || Journal::open(path, options)).await?;
        Ok(Self {
            inner: Arc::new(StoreInner {
                options,
                journal: Mutex::new(journal),
                key_locks: Mutex::new(HashMap::new()),
                lease_budget: Arc::new(Semaphore::new(options.max_leases)),
            }),
        })
    }

    /// 参数为完整身份与原始方向；返回有界排他租约，冲突或等待预算耗尽则拒绝。
    /// 所有使用同一个Store克隆的Dispatcher共享此排他权；等待可安全取消。
    pub async fn lease(
        &self,
        identity: BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchLease, RmError> {
        let budget_permit = self
            .inner
            .lease_budget
            .clone()
            .try_acquire_owned()
            .map_err(|_| RmError::Capacity {
                budget: "active and waiting leases",
            })?;
        let key_lock = {
            let mut locks = lock(&self.inner.key_locks)?;
            locks.retain(|_, value| value.strong_count() != 0);
            let key = identity.key();
            if let Some(existing) = locks.get(&key).and_then(Weak::upgrade) {
                existing
            } else {
                let value = Arc::new(Semaphore::new(1));
                locks.insert(key, Arc::downgrade(&value));
                value
            }
        };
        let key_permit = key_lock
            .acquire_owned()
            .await
            .map_err(|_| RmError::Unavailable {
                reason: "branch lease closed",
            })?;
        let inner = self.inner.clone();
        let expected = identity.clone();
        let guards = Arc::new(LeaseGuards {
            _key_permit: key_permit,
            _budget_permit: budget_permit,
            operation: Arc::new(tokio::sync::Mutex::new(())),
        });
        let worker_guards = guards.clone();
        let state = blocking(move || {
            let _guards = worker_guards;
            let journal = lock(&inner.journal)?;
            journal.ensure_healthy()?;
            journal.lookup(&expected, phase)
        })
        .await?;
        Ok(BranchLease {
            store: self.clone(),
            identity,
            phase,
            state,
            guards,
        })
    }

    pub(crate) async fn persist(
        &self,
        identity: BranchIdentity,
        phase: BranchPhase,
        state: BranchRecordState,
        guards: Arc<LeaseGuards>,
    ) -> Result<(), RmError> {
        let inner = self.inner.clone();
        // 锁在入队前取得并移入不可取消的磁盘任务，旧写不能晚于同租约的新一次写。
        let serial = guards.operation.clone().lock_owned().await;
        blocking(move || {
            let _serial = serial;
            let _guards = guards;
            lock(&inner.journal)?.append(identity, phase, state, inner.options)
        })
        .await
    }
}

fn lock<T>(value: &Mutex<T>) -> Result<MutexGuard<'_, T>, RmError> {
    value.lock().map_err(|_| RmError::Unavailable {
        reason: "shared journal state poisoned",
    })
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, RmError> + Send + 'static,
) -> Result<T, RmError> {
    tokio::runtime::Handle::try_current().map_err(|_| RmError::Unavailable {
        reason: "Tokio runtime required",
    })?;
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| RmError::Unavailable {
            reason: "journal worker failed",
        })?
}

impl Journal {
    fn open(path: PathBuf, options: JournalOptions) -> Result<Self, RmError> {
        let (mut file, created) = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => (file, true),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => (
                OpenOptions::new().read(true).write(true).open(&path)?,
                false,
            ),
            Err(error) => return Err(error.into()),
        };
        // 全生命周期仅持有此File；不克隆句柄、不解锁后继续读写。
        fs2::FileExt::try_lock_exclusive(&file)?;
        if created {
            file.write_all(HEADER)?;
            file.sync_all()?;
            #[cfg(unix)]
            if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
                File::open(parent)?.sync_all()?;
            }
        }
        let bytes = file.metadata()?.len();
        if bytes < HEADER.len() as u64 || bytes > options.max_file_bytes {
            return Err(RmError::Journal {
                reason: "invalid journal file length",
            });
        }
        file.seek(SeekFrom::Start(0))?;
        let mut header = [0; 8];
        file.read_exact(&mut header)?;
        if &header != HEADER {
            return Err(RmError::Journal {
                reason: "unsupported journal header/version",
            });
        }
        let mut journal = Self {
            file,
            entries: HashMap::new(),
            bytes,
            sequence: 0,
            faulted: false,
        };
        let mut consumed = HEADER.len() as u64;
        while consumed < bytes {
            if bytes - consumed < 4 {
                return Err(RmError::Journal {
                    reason: "partial record length",
                });
            }
            let mut prefix = [0; 4];
            journal.file.read_exact(&mut prefix)?;
            let length = u32::from_be_bytes(prefix) as usize;
            let total = length.checked_add(4).ok_or(RmError::Journal {
                reason: "record length overflow",
            })?;
            if total < RECORD_OVERHEAD
                || total > options.max_record_bytes
                || total as u64 > bytes - consumed
            {
                return Err(RmError::Journal {
                    reason: "record length outside file/budget",
                });
            }
            let mut record = vec![0; length];
            journal.file.read_exact(&mut record)?;
            let checksum_offset = length - 4;
            let expected =
                u32::from_be_bytes(record[checksum_offset..].try_into().map_err(|_| {
                    RmError::Journal {
                        reason: "checksum width",
                    }
                })?);
            if crc32fast::hash(&record[..checksum_offset]) != expected {
                return Err(RmError::Journal {
                    reason: "record checksum mismatch",
                });
            }
            let sequence =
                u64::from_be_bytes(record[..8].try_into().map_err(|_| RmError::Journal {
                    reason: "sequence width",
                })?);
            if journal.sequence.checked_add(1) != Some(sequence) {
                return Err(RmError::Journal {
                    reason: "nonsequential journal record",
                });
            }
            let state = BranchRecordState::decode(record[8])?;
            let (identity, phase) = decode_identity(&record[9..checksum_offset], options)?;
            let previous = journal.lookup(&identity, phase)?;
            if !BranchRecordState::permits(previous, state) {
                return Err(RmError::Journal {
                    reason: "invalid persisted transition",
                });
            }
            if previous.is_none() && journal.entries.len() >= options.max_branches {
                return Err(RmError::Capacity {
                    budget: "persisted branches",
                });
            }
            journal.entries.insert(
                identity.key(),
                StoredRecord {
                    identity,
                    phase,
                    state,
                },
            );
            journal.sequence = sequence;
            consumed += total as u64;
        }
        Ok(journal)
    }

    fn ensure_healthy(&self) -> Result<(), RmError> {
        if self.faulted {
            return Err(RmError::Journal {
                reason: "previous write/sync failed; reopen and verify before use",
            });
        }
        Ok(())
    }
    fn lookup(
        &self,
        identity: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<Option<BranchRecordState>, RmError> {
        match self.entries.get(&identity.key()) {
            Some(value) if false => {
                Err(RmError::IdentityConflict)
            }
            Some(value) => Ok(Some(value.state)),
            None => Ok(None),
        }
    }
    fn append(
        &mut self,
        identity: BranchIdentity,
        phase: BranchPhase,
        state: BranchRecordState,
        options: JournalOptions,
    ) -> Result<(), RmError> {
        self.ensure_healthy()?;
        let previous = self.lookup(&identity, phase)?;
        if !BranchRecordState::permits(previous, state) {
            return Err(RmError::Journal {
                reason: "invalid requested transition",
            });
        }
        if previous == Some(state) {
            return Ok(());
        }
        if previous.is_none() && self.entries.len() >= options.max_branches {
            return Err(RmError::Capacity {
                budget: "persisted branches",
            });
        }
        let sequence = self.sequence.checked_add(1).ok_or(RmError::Capacity {
            budget: "journal sequence",
        })?;
        let record = encode_record(&identity, phase, state, sequence, options)?;
        let next_bytes = self
            .bytes
            .checked_add(record.len() as u64)
            .filter(|value| *value <= options.max_file_bytes)
            .ok_or(RmError::Capacity {
                budget: "journal file bytes",
            })?;
        // 尺寸和状态预检全部通过后才写。写入/同步错误永久中止当前Store，不能追加到残尾。
        let result = (|| -> Result<(), RmError> {
            if self.file.seek(SeekFrom::End(0))? != self.bytes {
                return Err(RmError::Journal {
                    reason: "journal length changed outside owner",
                });
            }
            self.file.write_all(&record)?;
            self.file.sync_all()?;
            Ok(())
        })();
        if let Err(error) = result {
            self.faulted = true;
            return Err(error);
        }
        self.entries.insert(
            identity.key(),
            StoredRecord {
                identity,
                phase,
                state,
            },
        );
        self.sequence = sequence;
        self.bytes = next_bytes;
        Ok(())
    }
}

fn encode_record(
    identity: &BranchIdentity,
    phase: BranchPhase,
    state: BranchRecordState,
    sequence: u64,
    options: JournalOptions,
) -> Result<Vec<u8>, RmError> {
    let message = match phase {
        BranchPhase::Commit => Message::BranchCommitRequest(BranchCommitRequest {
            xid: identity.xid().into(),
            branch_id: identity.branch_id(),
            branch_type: identity.resource().branch_type(),
            resource_id: identity.resource().resource_id().into(),
            application_data: identity.application_data().map(str::to_owned),
        }),
        BranchPhase::Rollback => Message::BranchRollbackRequest(BranchRollbackRequest {
            xid: identity.xid().into(),
            branch_id: identity.branch_id(),
            branch_type: identity.resource().branch_type(),
            resource_id: identity.resource().resource_id().into(),
            application_data: identity.application_data().map(str::to_owned),
        }),
    };
    let payload = PayloadCodec::new(options.max_record_bytes - RECORD_OVERHEAD)
        .and_then(|codec| codec.encode(&message))
        .map_err(|_| RmError::Capacity {
            budget: "journal record bytes",
        })?;
    let length = payload.len() + RECORD_OVERHEAD - 4;
    let mut record = Vec::with_capacity(length + 4);
    record.extend_from_slice(&(length as u32).to_be_bytes());
    record.extend_from_slice(&sequence.to_be_bytes());
    record.push(state as u8);
    record.extend_from_slice(&payload);
    let checksum = crc32fast::hash(&record[4..]);
    record.extend_from_slice(&checksum.to_be_bytes());
    Ok(record)
}
fn decode_identity(
    payload: &[u8],
    options: JournalOptions,
) -> Result<(BranchIdentity, BranchPhase), RmError> {
    let message = PayloadCodec::new(options.max_record_bytes - RECORD_OVERHEAD)
        .and_then(|codec| codec.decode(payload))
        .map_err(|_| RmError::Journal {
            reason: "invalid identity payload",
        })?;
    let (xid, branch_id, branch_type, resource_id, application_data, phase) = match message {
        Message::BranchCommitRequest(value) => (
            value.xid,
            value.branch_id,
            value.branch_type,
            value.resource_id,
            value.application_data,
            BranchPhase::Commit,
        ),
        Message::BranchRollbackRequest(value) => (
            value.xid,
            value.branch_id,
            value.branch_type,
            value.resource_id,
            value.application_data,
            BranchPhase::Rollback,
        ),
        _ => {
            return Err(RmError::Journal {
                reason: "journal payload is not a branch end request",
            })
        }
    };
    let resource = ResourceKey::new(branch_type, resource_id).map_err(|_| RmError::Journal {
        reason: "invalid persisted resource",
    })?;
    let identity =
        BranchIdentity::new(xid, branch_id, resource, application_data).map_err(|_| {
            RmError::Journal {
                reason: "invalid persisted branch identity",
            }
        })?;
    Ok((identity, phase))
}

#[cfg(test)]
mod tests {
    use super::Journal;
    use crate::{BranchIdentity, BranchPhase, BranchRecordState, JournalOptions, RmError};
    use seata_protocol::BranchType;
    use seata_transport::ResourceKey;
    use std::fs::File;
    use std::path::PathBuf;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn actual_write_failure_poison_prevents_a_later_false_completion() {
        let folder = PathBuf::from(format!(
            "D:/codex-builds/rm-write-failure-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("branches.log");
        let options = JournalOptions::new(1, 65536, 4096, 1).unwrap();
        let identity = BranchIdentity::new(
            "xid".into(),
            1,
            ResourceKey::new(BranchType::At, "fixture".into()).unwrap(),
            None,
        )
        .unwrap();
        let mut journal = Journal::open(path.clone(), options).unwrap();
        journal
            .append(
                identity.clone(),
                BranchPhase::Commit,
                BranchRecordState::Pending,
                options,
            )
            .unwrap();
        // 测试内部替换为真实只读OS句柄制造write失败；生产代码从不更换或复制持锁句柄。
        journal.file = File::open(&path).unwrap();
        assert!(matches!(
            journal.append(
                identity.clone(),
                BranchPhase::Commit,
                BranchRecordState::Completed,
                options
            ),
            Err(RmError::Storage { .. })
        ));
        assert!(journal.faulted);
        assert_eq!(
            journal.lookup(&identity, BranchPhase::Commit).unwrap(),
            Some(BranchRecordState::Pending)
        );
        assert!(journal.ensure_healthy().is_err());
        assert!(journal
            .append(
                identity.clone(),
                BranchPhase::Commit,
                BranchRecordState::Completed,
                options
            )
            .is_err());
        drop(journal);
        assert_eq!(
            Journal::open(path, options)
                .unwrap()
                .lookup(&identity, BranchPhase::Commit)
                .unwrap(),
            Some(BranchRecordState::Pending)
        );
    }
}
