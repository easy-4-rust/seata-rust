use crate::{sea_orm_mysql_value::SeaOrmMySqlValue, SeaOrmMySqlError};
use seata_datasource::FieldValue;
/// DML 与只读计划共用的原序、原类型有界绑定转换。
/// 对应 Java: 无直接对象；Rust ORM PreparedStatement 参数转换扩展。
pub(crate) struct SeaOrmMySqlBindings;
impl SeaOrmMySqlBindings {
    /// 参数为拥有式 SeaORM 原值与精确占位符数量；返回完整原序绑定，不内插值。
    pub(crate) fn convert(
        values: Vec<sea_orm::Value>,
        expected: usize,
    ) -> Result<Vec<FieldValue>, SeaOrmMySqlError> {
        let original = values;
        if original.len() != expected || original.len() > 1024 {
            return Err(SeaOrmMySqlError::Input {
                reason: "exact bounded parameter count required",
            });
        }
        let mut total = 0_usize;
        let mut parameters = Vec::with_capacity(original.len());
        for value in original {
            let value = SeaOrmMySqlValue::convert(value)?;
            total = total
                .checked_add(value.value_bytes())
                .ok_or(SeaOrmMySqlError::Input {
                    reason: "parameter bytes overflow",
                })?;
            if total > 8_388_608 {
                return Err(SeaOrmMySqlError::Input {
                    reason: "parameter total budget",
                });
            }
            parameters.push(value);
        }
        Ok(parameters)
    }
}
