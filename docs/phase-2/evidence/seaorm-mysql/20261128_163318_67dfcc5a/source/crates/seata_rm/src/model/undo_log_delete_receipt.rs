use crate::RmError;
use seata_protocol::{transaction::UndoLogDeleteRequest, BranchType};
use seata_transport::ResourceKey;

/// 原已登记通道收到的单向清理能力；不可由业务调用方从字段手工构造。
/// 对应 Java: RMHandlerAT#handle(UndoLogDeleteRequest) 的 Rust 来源约束扩展。
#[derive(Debug)]
pub struct UndoLogDeleteReceipt {
    resource: ResourceKey,
    save_days: u16,
}
impl UndoLogDeleteReceipt {
    pub(crate) fn received(request: &UndoLogDeleteRequest) -> Result<Self, RmError> {
        if request.branch_type != BranchType::At {
            return Err(RmError::Invalid {
                reason: "Undo cleanup requires AT",
            });
        }
        let resource =
            ResourceKey::new(request.branch_type, request.resource_id.clone()).map_err(|_| {
                RmError::Invalid {
                    reason: "invalid Undo cleanup resource",
                }
            })?;
        Ok(Self {
            resource,
            save_days: u16::try_from(request.save_days)
                .ok()
                .filter(|days| *days > 0)
                .unwrap_or(7),
        })
    }
    /// 无参数；返回已匹配的原资源键，不提供可变引用。
    pub fn resource(&self) -> &ResourceKey {
        &self.resource
    }
    /// 无参数；返回按原 Java 规则处理非正输入后的保留天数。
    pub const fn save_days(&self) -> u16 {
        self.save_days
    }
}

#[cfg(test)]
mod tests {
    use super::UndoLogDeleteReceipt;
    use seata_protocol::{transaction::UndoLogDeleteRequest, BranchType};
    #[test]
    fn defaults_and_exact_resource_match_the_original_request() {
        for days in [i16::MIN, -1, 0, 1, 7, i16::MAX] {
            let request = UndoLogDeleteRequest {
                branch_type: BranchType::At,
                resource_id: "jdbc:mysql://HOST/db".into(),
                save_days: days,
            };
            let receipt =
                UndoLogDeleteReceipt::received(&request).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(receipt.resource().resource_id(), request.resource_id);
            assert_eq!(receipt.save_days(), if days <= 0 { 7 } else { days as u16 });
        }
    }
    #[test]
    fn other_modes_and_invalid_resource_do_not_receive_cleanup_authority() {
        for (kind, resource) in [
            (BranchType::Tcc, "db"),
            (BranchType::Xa, "db"),
            (BranchType::At, ""),
            (BranchType::At, "a,b"),
        ] {
            assert!(UndoLogDeleteReceipt::received(&UndoLogDeleteRequest {
                branch_type: kind,
                resource_id: resource.into(),
                save_days: 7
            })
            .is_err());
        }
    }
}
