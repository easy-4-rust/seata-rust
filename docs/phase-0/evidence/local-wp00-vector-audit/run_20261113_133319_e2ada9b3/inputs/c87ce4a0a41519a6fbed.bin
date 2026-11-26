use std::io;
use std::path::Path;

/// 跨测试客户端进程保留的 TC 身份，不是 TC 会话存储或恢复实现。
/// 对应 Java: GlobalSession / BranchSession 的标识；仅保存测试观测，不移植其存储逻辑。
#[derive(Debug, PartialEq)]
pub(super) struct RestartRecord {
    pub(super) xid: String,
    pub(super) branch_id: i64,
    pub(super) resource: String,
}
impl RestartRecord {
    /// 解码版本化文本，返回原 XID、分支 ID（0 表示无分支）和资源，格式错误时拒绝。
    pub(super) fn decode(text: &str) -> io::Result<Self> {
        let fields: Vec<&str> = text.lines().collect();
        if text.len() > 65536 || fields.len() != 4 || fields[0] != "seata_it_restart_v1" {
            return Err(crate::invalid("invalid restart record version or length"));
        }
        let id = fields[2]
            .parse::<i64>()
            .map_err(|_| crate::invalid("invalid recorded BranchId"))?;
        if id < 0
            || id.to_string() != fields[2]
            || [fields[1], fields[3]].iter().any(|value| {
                value.is_empty() || value.trim() != *value || value.chars().any(char::is_control)
            })
        {
            return Err(crate::invalid("invalid recorded identity"));
        }
        Ok(Self {
            xid: fields[1].to_owned(),
            branch_id: id,
            resource: fields[3].to_owned(),
        })
    }
    /// 将记录以 create_new 写入给定路径并同步；禁止覆盖已有记录。
    pub(super) fn save(&self, path: &Path) -> io::Result<()> {
        use std::io::Write;
        let text = format!(
            "seata_it_restart_v1\n{}\n{}\n{}\n",
            self.xid, self.branch_id, self.resource
        );
        if Self::decode(&text)? != *self {
            return Err(crate::invalid("identity cannot be represented in record"));
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()
    }
    /// 从给定路径读取有大小上限的记录并严格解码，返回身份或错误。
    pub(super) fn load(path: &Path) -> io::Result<Self> {
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(path)?
            .take(65537)
            .read_to_string(&mut text)?;
        Self::decode(&text)
    }
}
#[cfg(test)]
mod tests {
    use super::RestartRecord;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    #[test]
    fn decodes_original_identity_with_or_without_a_branch() {
        for id in [0, 42] {
            assert_eq!(
                RestartRecord::decode(&format!("seata_it_restart_v1\nx\n{id}\nmysql://orders\n"))
                    .unwrap(),
                RestartRecord {
                    xid: "x".into(),
                    branch_id: id,
                    resource: "mysql://orders".into()
                }
            );
        }
    }
    #[test]
    fn malformed_record_is_not_usable_as_a_transaction() {
        for text in [
            "",
            "v2\nx\n42\nr\n",
            "seata_it_restart_v1\n\n42\nr\n",
            "seata_it_restart_v1\nx\n-1\nr\n",
            "seata_it_restart_v1\nx\n9223372036854775808\nr\n",
            "seata_it_restart_v1\nx\n1\nr\nextra\n",
        ] {
            assert!(RestartRecord::decode(text).is_err());
        }
    }
    #[test]
    fn identity_is_synced_and_never_overwrites_existing_record() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!(
            "restart_record_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let record = RestartRecord {
            xid: "x".into(),
            branch_id: 42,
            resource: "r".into(),
        };
        record.save(&path).unwrap();
        assert_eq!(RestartRecord::load(&path).unwrap(), record);
        assert_eq!(
            record.save(&path).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(RestartRecord::load(&path).unwrap(), record);
        std::fs::remove_file(path).unwrap();
    }
}
