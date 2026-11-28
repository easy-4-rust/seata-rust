use crate::DbError;
use seata_at::MySqlJdbcProfile;
use sqlx::{MySql, Transaction};

/// 由部署方在每个业务 schema 预置的唯一 JDBC 资源标识、可选 TINYINT(1) 映射及完整 JDBC 配置。
/// 对应 Java: 无直接对象；校验原 TC 以 JDBC 标识划分锁域的 Rust 扩展。
pub(crate) struct MySqlResourceBinding;

impl MySqlResourceBinding {
    pub(crate) const TABLE: &'static str = "seata_rs_at_resource_binding";

    /// 参数为原业务事务、JDBC 标识、可选映射及完整配置要求；返回核验结果。成功后共享行锁与 MDL 由原事务持有。
    pub(crate) async fn lock_verified(
        transaction: &mut Transaction<'static, MySql>,
        resource_id: &str,
        profile: Option<MySqlJdbcProfile>,
        full_profile: bool,
    ) -> Result<(), DbError> {
        let rows: Vec<Vec<u8>> = sqlx::query_scalar(
            "SELECT resource_id FROM seata_rs_at_resource_binding WHERE slot=1 LIMIT 2 FOR SHARE",
        )
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| DbError::driver("read AT resource binding", error))?;
        if !matches!(rows.as_slice(), [bound] if bound == resource_id.as_bytes()) {
            return Err(DbError::Input {
                reason: "MySQL AT resource binding is missing or differs",
            });
        }

        // 先读真实表以持有 MDL，再校验物理表；非 InnoDB 或视图不能提供本地事务行锁。
        let is_innodb: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.TABLES \
             WHERE BINARY TABLE_SCHEMA=BINARY DATABASE() \
             AND BINARY TABLE_NAME='seata_rs_at_resource_binding' \
             AND TABLE_TYPE='BASE TABLE' AND ENGINE='InnoDB'",
        )
        .fetch_one(&mut **transaction)
        .await
        .map_err(|error| DbError::driver("identify AT resource binding", error))?;
        if is_innodb != 1 {
            return Err(DbError::Unsupported {
                feature: "AT resource binding requires an InnoDB base table",
            });
        }
        // 原事务可能早已读取其他业务表的字典快照；必须在当前绑定表 MDL 下复核实际引擎。
        crate::cache::MySqlTableMetaReader::verify_current_engine(transaction, Self::TABLE).await?;
        if full_profile && profile.is_none() {
            return Err(DbError::Input {
                reason: "MySQL AT full JDBC profile is required",
            });
        }
        if let Some(profile) = profile {
            let mapped: Option<u8> = sqlx::query_scalar(
                "SELECT tinyint1_is_bit FROM seata_rs_at_resource_binding WHERE slot=1 FOR SHARE",
            )
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| DbError::driver("read AT TINYINT(1) profile binding", error))?;
            let expected = u8::from(profile.boolean_tinyint());
            if mapped != Some(expected) {
                return Err(DbError::Input {
                    reason: "MySQL AT TINYINT(1) profile binding is missing or differs",
                });
            }
            if full_profile {
                let declared: Option<u8> = sqlx::query_scalar(
                    "SELECT jdbc_profile FROM seata_rs_at_resource_binding WHERE slot=1 FOR SHARE",
                )
                .fetch_one(&mut **transaction)
                .await
                .map_err(|error| DbError::driver("read AT JDBC profile binding", error))?;
                if declared != Some(profile.storage_code()) {
                    return Err(DbError::Input {
                        reason: "MySQL AT JDBC profile binding is missing or differs",
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "mysql_resource_binding_jdbc_tests.rs"]
mod jdbc_tests;
