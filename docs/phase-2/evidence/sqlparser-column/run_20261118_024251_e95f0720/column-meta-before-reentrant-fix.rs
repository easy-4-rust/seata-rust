use crate::{JavaObject, JavaString, MetadataError};
use std::{
    any::Any,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// 列元数据共享句柄；Clone 保留同一 Java 对象的引用身份和后续修改。
/// 对应 Java: org.apache.seata.sqlparser.struct.ColumnMeta
#[derive(Debug, Clone, Default)]
pub struct ColumnMeta {
    state: Arc<RwLock<ColumnMetaState>>,
}

/// ColumnMeta 的内部状态，不是独立迁移对象。
#[derive(Debug, Clone, Default)]
struct ColumnMetaState {
    table_cat: Option<JavaString>,
    table_schema_name: Option<JavaString>,
    table_name: Option<JavaString>,
    column_name: Option<JavaString>,
    data_type: i32,
    data_type_name: Option<JavaString>,
    column_size: i32,
    decimal_digits: i32,
    num_prec_radix: i32,
    null_able: i32,
    remarks: Option<JavaString>,
    column_def: Option<JavaString>,
    sql_data_type: i32,
    sql_datetime_sub: i32,
    char_octet_length: Option<Arc<dyn JavaObject>>,
    ordinal_position: i32,
    is_null_able: Option<JavaString>,
    is_autoincrement: Option<JavaString>,
    is_on_update: bool,
    is_case_sensitive: bool,
}

impl ColumnMeta {
    fn read(&self) -> Result<RwLockReadGuard<'_, ColumnMetaState>, MetadataError> {
        self.state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("ColumnMeta"))
    }

    fn write(&self) -> Result<RwLockWriteGuard<'_, ColumnMetaState>, MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("ColumnMeta"))
    }

    /// 返回自增标志；无参数，原文本按大小写不敏感的YES比较，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#isAutoincrement
    pub fn is_autoincrement(&self) -> Result<bool, MetadataError> {
        Ok(self.read()?.is_autoincrement.as_ref().is_some_and(|text| {
            // 原JDK17对YES的全部等价码元；包含ASCII比较遗漏的长s U+017F。
            matches!(
                text.as_utf16(),
                [0x59 | 0x79, 0x45 | 0x65, 0x53 | 0x73 | 0x17f]
            )
        }))
    }

    /// 返回表目录；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getTableCat
    pub fn table_cat(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.table_cat.clone())
    }

    /// 用 table_cat 替换表目录；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setTableCat
    pub fn set_table_cat(&self, table_cat: Option<JavaString>) -> Result<(), MetadataError> {
        self.write()?.table_cat = table_cat;
        Ok(())
    }

    /// 返回表模式名；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getTableSchemaName
    pub fn table_schema_name(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.table_schema_name.clone())
    }

    /// 用 table_schema_name 替换表模式名；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setTableSchemaName
    pub fn set_table_schema_name(
        &self,
        table_schema_name: Option<JavaString>,
    ) -> Result<(), MetadataError> {
        self.write()?.table_schema_name = table_schema_name;
        Ok(())
    }

    /// 返回表名；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getTableName
    pub fn table_name(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.table_name.clone())
    }

    /// 用 table_name 替换表名；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setTableName
    pub fn set_table_name(&self, table_name: Option<JavaString>) -> Result<(), MetadataError> {
        self.write()?.table_name = table_name;
        Ok(())
    }

    /// 返回列名；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getColumnName
    pub fn column_name(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.column_name.clone())
    }

    /// 用 column_name 替换列名；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setColumnName
    pub fn set_column_name(&self, column_name: Option<JavaString>) -> Result<(), MetadataError> {
        self.write()?.column_name = column_name;
        Ok(())
    }

    /// 返回JDBC类型编号；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getDataType
    pub fn data_type(&self) -> Result<i32, MetadataError> {
        Ok(self.read()?.data_type)
    }

    /// 用 data_type 替换JDBC类型编号；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setDataType
    pub fn set_data_type(&self, data_type: i32) -> Result<(), MetadataError> {
        self.write()?.data_type = data_type;
        Ok(())
    }

    /// 返回数据库类型名；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getDataTypeName
    pub fn data_type_name(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.data_type_name.clone())
    }

    /// 用 data_type_name 替换数据库类型名；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setDataTypeName
    pub fn set_data_type_name(
        &self,
        data_type_name: Option<JavaString>,
    ) -> Result<(), MetadataError> {
        self.write()?.data_type_name = data_type_name;
        Ok(())
    }

    /// 返回列长度；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getColumnSize
    pub fn column_size(&self) -> Result<i32, MetadataError> {
        Ok(self.read()?.column_size)
    }

    /// 用 column_size 替换列长度；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setColumnSize
    pub fn set_column_size(&self, column_size: i32) -> Result<(), MetadataError> {
        self.write()?.column_size = column_size;
        Ok(())
    }

    /// 返回小数位数；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getDecimalDigits
    pub fn decimal_digits(&self) -> Result<i32, MetadataError> {
        Ok(self.read()?.decimal_digits)
    }

    /// 用 decimal_digits 替换小数位数；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setDecimalDigits
    pub fn set_decimal_digits(&self, decimal_digits: i32) -> Result<(), MetadataError> {
        self.write()?.decimal_digits = decimal_digits;
        Ok(())
    }

    /// 返回精度基数；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getNumPrecRadix
    pub fn num_prec_radix(&self) -> Result<i32, MetadataError> {
        Ok(self.read()?.num_prec_radix)
    }

    /// 用 num_prec_radix 替换精度基数；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setNumPrecRadix
    pub fn set_num_prec_radix(&self, num_prec_radix: i32) -> Result<(), MetadataError> {
        self.write()?.num_prec_radix = num_prec_radix;
        Ok(())
    }

    /// 返回JDBC可空性编号；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getNullAble
    pub fn null_able(&self) -> Result<i32, MetadataError> {
        Ok(self.read()?.null_able)
    }

    /// 用 null_able 替换JDBC可空性编号；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setNullAble
    pub fn set_null_able(&self, null_able: i32) -> Result<(), MetadataError> {
        self.write()?.null_able = null_able;
        Ok(())
    }

    /// 返回备注；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getRemarks
    pub fn remarks(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.remarks.clone())
    }

    /// 用 remarks 替换备注；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setRemarks
    pub fn set_remarks(&self, remarks: Option<JavaString>) -> Result<(), MetadataError> {
        self.write()?.remarks = remarks;
        Ok(())
    }

    /// 返回默认值；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getColumnDef
    pub fn column_def(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.column_def.clone())
    }

    /// 用 column_def 替换默认值；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setColumnDef
    pub fn set_column_def(&self, column_def: Option<JavaString>) -> Result<(), MetadataError> {
        self.write()?.column_def = column_def;
        Ok(())
    }

    /// 返回SQL类型编号；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getSqlDataType
    pub fn sql_data_type(&self) -> Result<i32, MetadataError> {
        Ok(self.read()?.sql_data_type)
    }

    /// 用 sql_data_type 替换SQL类型编号；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setSqlDataType
    pub fn set_sql_data_type(&self, sql_data_type: i32) -> Result<(), MetadataError> {
        self.write()?.sql_data_type = sql_data_type;
        Ok(())
    }

    /// 返回日期时间子类型编号；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getSqlDatetimeSub
    pub fn sql_datetime_sub(&self) -> Result<i32, MetadataError> {
        Ok(self.read()?.sql_datetime_sub)
    }

    /// 用 sql_datetime_sub 替换日期时间子类型编号；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setSqlDatetimeSub
    pub fn set_sql_datetime_sub(&self, sql_datetime_sub: i32) -> Result<(), MetadataError> {
        self.write()?.sql_datetime_sub = sql_datetime_sub;
        Ok(())
    }

    /// 返回字符字节长度对象；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getCharOctetLength
    pub fn char_octet_length(&self) -> Result<Option<Arc<dyn JavaObject>>, MetadataError> {
        Ok(self.read()?.char_octet_length.clone())
    }

    /// 用 char_octet_length 替换字符字节长度对象；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setCharOctetLength
    pub fn set_char_octet_length(
        &self,
        char_octet_length: Option<Arc<dyn JavaObject>>,
    ) -> Result<(), MetadataError> {
        // 先释放元数据锁，再释放旧对象，避免用户Drop回调在锁内重入。
        let previous = {
            let mut state = self.write()?;
            std::mem::replace(&mut state.char_octet_length, char_octet_length)
        };
        drop(previous);
        Ok(())
    }

    /// 返回列序号；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getOrdinalPosition
    pub fn ordinal_position(&self) -> Result<i32, MetadataError> {
        Ok(self.read()?.ordinal_position)
    }

    /// 用 ordinal_position 替换列序号；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setOrdinalPosition
    pub fn set_ordinal_position(&self, ordinal_position: i32) -> Result<(), MetadataError> {
        self.write()?.ordinal_position = ordinal_position;
        Ok(())
    }

    /// 返回原可空性文本；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getIsNullAble
    pub fn is_null_able_text(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.is_null_able.clone())
    }

    /// 用 is_null_able 替换原可空性文本；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setIsNullAble
    pub fn set_is_null_able(&self, is_null_able: Option<JavaString>) -> Result<(), MetadataError> {
        self.write()?.is_null_able = is_null_able;
        Ok(())
    }

    /// 返回原自增文本；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#getIsAutoincrement
    pub fn is_autoincrement_text(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.read()?.is_autoincrement.clone())
    }

    /// 用 is_autoincrement 替换原自增文本；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setIsAutoincrement
    pub fn set_is_autoincrement(
        &self,
        is_autoincrement: Option<JavaString>,
    ) -> Result<(), MetadataError> {
        self.write()?.is_autoincrement = is_autoincrement;
        Ok(())
    }

    /// 返回自动更新标志；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#isOnUpdate
    pub fn is_on_update(&self) -> Result<bool, MetadataError> {
        Ok(self.read()?.is_on_update)
    }

    /// 用 on_update 替换自动更新标志；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setOnUpdate
    pub fn set_on_update(&self, on_update: bool) -> Result<(), MetadataError> {
        self.write()?.is_on_update = on_update;
        Ok(())
    }

    /// 返回大小写敏感标志；无参数，引用值保留null，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#isCaseSensitive
    pub fn is_case_sensitive(&self) -> Result<bool, MetadataError> {
        Ok(self.read()?.is_case_sensitive)
    }

    /// 用 case_sensitive 替换大小写敏感标志；成功返回()，锁中毒返回错误。
    /// 对应 Java: ColumnMeta#setCaseSensitive
    pub fn set_case_sensitive(&self, case_sensitive: bool) -> Result<(), MetadataError> {
        self.write()?.is_case_sensitive = case_sensitive;
        Ok(())
    }

    /// 比较可空动态对象other；同一引用立即为true，类型不同为false，读取失败返回错误。
    /// 对应 Java: ColumnMeta#equals；保留原方法other字段先调用equals的方向。
    pub fn java_equals(&self, other: Option<&dyn Any>) -> Result<bool, MetadataError> {
        let Some(other) = other.and_then(|value| value.downcast_ref::<Self>()) else {
            return Ok(false);
        };
        if Arc::ptr_eq(&self.state, &other.state) {
            return Ok(true);
        }
        let mine = self.read()?.clone();
        let theirs = other.read()?.clone();
        // 两份快照均已释放锁，用户对象equals可以安全重入元数据访问。
        if theirs.table_cat != mine.table_cat {
            return Ok(false);
        }
        if theirs.table_schema_name != mine.table_schema_name {
            return Ok(false);
        }
        if theirs.table_name != mine.table_name {
            return Ok(false);
        }
        if theirs.column_name != mine.column_name {
            return Ok(false);
        }
        if theirs.data_type != mine.data_type {
            return Ok(false);
        }
        if theirs.data_type_name != mine.data_type_name {
            return Ok(false);
        }
        if theirs.column_size != mine.column_size {
            return Ok(false);
        }
        if theirs.decimal_digits != mine.decimal_digits {
            return Ok(false);
        }
        if theirs.num_prec_radix != mine.num_prec_radix {
            return Ok(false);
        }
        if theirs.null_able != mine.null_able {
            return Ok(false);
        }
        if theirs.remarks != mine.remarks {
            return Ok(false);
        }
        if theirs.column_def != mine.column_def {
            return Ok(false);
        }
        if theirs.sql_data_type != mine.sql_data_type {
            return Ok(false);
        }
        if theirs.sql_datetime_sub != mine.sql_datetime_sub {
            return Ok(false);
        }
        let same_object = match (&theirs.char_octet_length, &mine.char_octet_length) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                Arc::ptr_eq(left, right) || left.java_equals(right.as_ref())
            }
            _ => false,
        };
        if !same_object {
            return Ok(false);
        }
        if theirs.ordinal_position != mine.ordinal_position {
            return Ok(false);
        }
        if theirs.is_null_able != mine.is_null_able {
            return Ok(false);
        }
        if theirs.is_autoincrement != mine.is_autoincrement {
            return Ok(false);
        }
        if theirs.is_on_update != mine.is_on_update {
            return Ok(false);
        }
        if theirs.is_case_sensitive != mine.is_case_sensitive {
            return Ok(false);
        }
        Ok(true)
    }

    /// 返回Java的字段哈希和；无参数，按i32回绕，读取失败返回错误。
    /// 对应 Java: ColumnMeta#hashCode；不使用Rust随机Hash替代。
    pub fn java_hash_code(&self) -> Result<i32, MetadataError> {
        let state = self.read()?.clone();
        let mut hash = 0_i32;
        hash = hash.wrapping_add(
            state
                .table_cat
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(
            state
                .table_schema_name
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(
            state
                .table_name
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(
            state
                .column_name
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(state.data_type);
        hash = hash.wrapping_add(
            state
                .data_type_name
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(state.column_size);
        hash = hash.wrapping_add(state.decimal_digits);
        hash = hash.wrapping_add(state.num_prec_radix);
        hash = hash.wrapping_add(state.null_able);
        hash = hash.wrapping_add(state.remarks.as_ref().map_or(0, JavaString::java_hash_code));
        hash = hash.wrapping_add(
            state
                .column_def
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(state.sql_data_type);
        hash = hash.wrapping_add(state.sql_datetime_sub);
        hash = hash.wrapping_add(
            state
                .char_octet_length
                .as_ref()
                .map_or(0, |value| value.java_hash_code()),
        );
        hash = hash.wrapping_add(state.ordinal_position);
        hash = hash.wrapping_add(
            state
                .is_null_able
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(
            state
                .is_autoincrement
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(if state.is_on_update { 1231 } else { 1237 });
        hash = hash.wrapping_add(if state.is_case_sensitive { 1231 } else { 1237 });
        Ok(hash)
    }

    /// 返回原字段顺序的完整Java文本；无参数，UTF-16无损，读取失败返回错误。
    /// 对应 Java: ColumnMeta#toString；null对象及其null的toString均输出字面量null。
    pub fn java_to_string(&self) -> Result<JavaString, MetadataError> {
        let state = self.read()?.clone();
        let mut units = Vec::new();
        units.extend("ColumnMeta{tableCat='".encode_utf16());
        units.extend(state.table_cat.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", tableSchemaName='".encode_utf16());
        units.extend(state.table_schema_name.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", tableName='".encode_utf16());
        units.extend(state.table_name.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", columnName='".encode_utf16());
        units.extend(state.column_name.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", dataType=".encode_utf16());
        units.extend(state.data_type.to_string().encode_utf16());
        units.extend(", dataTypeName='".encode_utf16());
        units.extend(state.data_type_name.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", columnSize=".encode_utf16());
        units.extend(state.column_size.to_string().encode_utf16());
        units.extend(", decimalDigits=".encode_utf16());
        units.extend(state.decimal_digits.to_string().encode_utf16());
        units.extend(", numPrecRadix=".encode_utf16());
        units.extend(state.num_prec_radix.to_string().encode_utf16());
        units.extend(", nullAble=".encode_utf16());
        units.extend(state.null_able.to_string().encode_utf16());
        units.extend(", remarks='".encode_utf16());
        units.extend(state.remarks.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", columnDef='".encode_utf16());
        units.extend(state.column_def.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", sqlDataType=".encode_utf16());
        units.extend(state.sql_data_type.to_string().encode_utf16());
        units.extend(", sqlDatetimeSub=".encode_utf16());
        units.extend(state.sql_datetime_sub.to_string().encode_utf16());
        units.extend(", charOctetLength=".encode_utf16());
        units.extend(
            state
                .char_octet_length
                .as_ref()
                .and_then(|value| value.java_to_string())
                .map_or_else(
                    || "null".encode_utf16().collect::<Vec<_>>(),
                    |value| value.as_utf16().to_vec(),
                ),
        );
        units.extend(", ordinalPosition=".encode_utf16());
        units.extend(state.ordinal_position.to_string().encode_utf16());
        units.extend(", isNullAble='".encode_utf16());
        units.extend(state.is_null_able.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", isAutoincrement='".encode_utf16());
        units.extend(state.is_autoincrement.as_ref().map_or_else(
            || "null".encode_utf16().collect::<Vec<_>>(),
            |value| value.as_utf16().to_vec(),
        ));
        units.push(39);
        units.extend(", isOnUpdate=".encode_utf16());
        units.extend(state.is_on_update.to_string().encode_utf16());
        units.extend(", isCaseSensitive=".encode_utf16());
        units.extend(state.is_case_sensitive.to_string().encode_utf16());
        units.push(125);
        Ok(JavaString::from_utf16(units))
    }
}
