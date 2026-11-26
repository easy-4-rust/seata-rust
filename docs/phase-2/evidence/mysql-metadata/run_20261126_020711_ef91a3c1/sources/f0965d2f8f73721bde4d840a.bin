use seata_at::LockKeyError;

/// 固定 Java TC 原版 MySQL 锁存储的字段长度约束。
/// 对应 Java: org.apache.seata.core.lock.AbstractLocker#getRowKey 与服务端 MySQL lock_table。
pub(crate) struct JavaTcMysqlLockLimits;

impl JavaTcMysqlLockLimits {
    /// 参数为原 JDBC 资源 ID 和已经通过行语法校验的锁键，按 utf8mb4 字符数核对每一行。
    /// 成功时保留原请求字节。错误仅报告违反的固定字段上限。
    pub(crate) fn verify(resource_id: &str, lock_key: &str) -> Result<(), LockKeyError> {
        let resource_chars = resource_id.chars().count();
        if resource_chars > 256 {
            return Err(LockKeyError::Budget {
                limit: "Java TC MySQL lock_table.resource_id (256 characters)",
            });
        }
        for group in lock_key.split(';') {
            let (table, rows) = group.split_once(':').ok_or(LockKeyError::Invalid {
                reason: "grouped AT registration lock key",
            })?;
            let table_chars = table.chars().count();
            if table_chars > 32 {
                return Err(LockKeyError::Budget {
                    limit: "Java TC MySQL lock_table.table_name (32 characters)",
                });
            }
            // 原 TC 使用 resource_id^^^table_name^^^pk。字节数和 Java UTF-16 长度均不等于列字符数。
            let row_prefix_chars = resource_chars + table_chars + 2 * "^^^".len();
            for pk in rows.split(',') {
                let pk_chars = pk.chars().count();
                if pk_chars > 36 {
                    return Err(LockKeyError::Budget {
                        limit: "Java TC MySQL lock_table.pk (36 characters)",
                    });
                }
                if row_prefix_chars + pk_chars > 128 {
                    return Err(LockKeyError::Budget {
                        limit: "Java TC MySQL lock_table.row_key (128 characters)",
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::JavaTcMysqlLockLimits;
    use seata_at::LockKeyError;

    #[test]
    fn each_group_and_row_gets_its_own_budget_including_pk_colons() {
        let resource = "r".repeat(86);
        let maximum = "🦀".repeat(35) + ":";
        let maximum_keys = format!("t:a,{maximum};t:b,{maximum}");
        assert_eq!(
            JavaTcMysqlLockLimits::verify(&resource, &maximum_keys),
            Err(LockKeyError::Budget {
                limit: "Java TC MySQL lock_table.row_key (128 characters)",
            })
        );
        let resource = "r".repeat(85);
        assert_eq!(
            JavaTcMysqlLockLimits::verify(&resource, &maximum_keys),
            Ok(())
        );
        let overflow = format!("t:a;t:b,{maximum}🦀");
        assert_eq!(
            JavaTcMysqlLockLimits::verify(&resource, &overflow),
            Err(LockKeyError::Budget {
                limit: "Java TC MySQL lock_table.pk (36 characters)",
            })
        );
    }
}
