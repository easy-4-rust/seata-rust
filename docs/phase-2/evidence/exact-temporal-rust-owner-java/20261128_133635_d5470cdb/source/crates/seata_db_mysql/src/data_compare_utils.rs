use crate::{
    JavaCompareField, JavaCompareRecords, JavaCompareRecordsMut, JavaCompareTemporal,
    JavaCompareValue,
};
use seata_datasource::{DecimalValue, FieldValue, Row, TableRecords};
use seata_sqlparser::{JavaObjectError, JavaString, MetadataError, MetadataLocale, PrimaryKeyMap};
use std::{collections::HashMap, sync::Arc};

#[cfg(test)]
#[path = "data_compare_utils_tests.rs"]
mod tests;
/// 原 Java 字符串主键到字段映射。
/// 对应 Java: DataCompareUtils#rowListToMap 的返回类型。
pub type JavaCompareRowMap<'a> = HashMap<JavaString, HashMap<JavaString, &'a JavaCompareField>>;

type JavaRowIndex = PrimaryKeyMap<(usize, PrimaryKeyMap<usize>)>;

/// 比较原 Java 对象及 MySQL 完整物理镜像。
/// 对应 Java: org.apache.seata.rm.datasource.DataCompareUtils（部分实现）。
pub struct DataCompareUtils;
impl DataCompareUtils {
    /// 比较两个可空的原字段，返回列名、JDBC 类型和值是否相等。
    /// 对应 Java: DataCompareUtils#isFieldEquals 无 serializer 转换路径。
    pub fn java_fields_equal(
        left: Option<&JavaCompareField>,
        right: Option<&JavaCompareField>,
    ) -> bool {
        match (left, right) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                Self::field_shape_equal(left, right) && left.value == right.value
            }
            _ => false,
        }
    }

    /// 比较两个可空 Fastjson 字段。名称、类型与值均非空时，按原顺序就地转换后比较。
    /// 日期时间由同一配置的宿主提供等价表示，转换失败返回原宿主错误并保留已经完成的变更。
    /// 对应 Java: DataCompareUtils#isFieldEquals / convertType。
    pub fn java_fastjson_fields_equal(
        left: Option<&mut JavaCompareField>,
        right: Option<&mut JavaCompareField>,
        mut parse_temporal: impl FnMut(
            JavaCompareTemporal,
            &JavaString,
        ) -> Result<JavaString, MetadataError>,
    ) -> Result<bool, MetadataError> {
        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            (None, None) => return Ok(true),
            _ => return Ok(false),
        };
        if !Self::field_shape_equal(left, right) {
            return Ok(false);
        }
        if matches!(left.value, JavaCompareValue::Null)
            || matches!(right.value, JavaCompareValue::Null)
        {
            return Ok(left.value == right.value);
        }
        let local = matches!(
            right.value,
            JavaCompareValue::Temporal(JavaCompareTemporal::LocalDateTime, _)
        );
        Self::convert_fastjson(left, local, &mut parse_temporal)?;
        Self::convert_fastjson(right, false, &mut parse_temporal)?;
        Ok(left.value == right.value)
    }

    fn convert_fastjson(
        field: &mut JavaCompareField,
        local_timestamp: bool,
        parse: &mut impl FnMut(JavaCompareTemporal, &JavaString) -> Result<JavaString, MetadataError>,
    ) -> Result<(), MetadataError> {
        let value = match (field.jdbc_type, &field.value) {
            (3, JavaCompareValue::Integer(value)) => {
                JavaCompareValue::Decimal(DecimalValue::new(&value.to_string()).map_err(|_| {
                    MetadataError::InvalidCompatibilityData("Fastjson Integer decimal")
                })?)
            }
            (-5, JavaCompareValue::Integer(value)) => JavaCompareValue::Long(i64::from(*value)),
            (91..=93, JavaCompareValue::Text(value)) => {
                let kind = match field.jdbc_type {
                    91 => JavaCompareTemporal::Date,
                    92 => JavaCompareTemporal::Time,
                    _ if local_timestamp => JavaCompareTemporal::LocalDateTime,
                    _ => JavaCompareTemporal::Timestamp,
                };
                let day;
                let input = if kind == JavaCompareTemporal::Date {
                    let units = value.as_utf16();
                    let space = u16::from(b' ');
                    // Java split 删除尾部空串。非空输入全为空格时，取第一个分段会越界。
                    if !units.is_empty() && units.iter().all(|unit| *unit == space) {
                        return Err(MetadataError::ObjectException(Arc::new(
                            JavaObjectError::new(
                                "java.lang.ArrayIndexOutOfBoundsException",
                                Some("Index 0 out of bounds for length 0".into()),
                            )
                            .with_superclasses([
                                "java.lang.IndexOutOfBoundsException",
                                "java.lang.RuntimeException",
                                "java.lang.Exception",
                                "java.lang.Throwable",
                            ]),
                        )));
                    }
                    let first_space = units.iter().position(|unit| *unit == space);
                    day = JavaString::from_utf16(
                        units[..first_space.unwrap_or(units.len())].to_vec(),
                    );
                    &day
                } else {
                    value
                };
                JavaCompareValue::Temporal(kind, parse(kind, input)?)
            }
            _ => return Ok(()),
        };
        field.value = value;
        Ok(())
    }

    fn field_shape_equal(left: &JavaCompareField, right: &JavaCompareField) -> bool {
        let names = match (&left.name, &right.name) {
            (None, None) => true,
            (Some(left), Some(right)) => Self::names_equal(left, right),
            _ => false,
        };
        names && left.jdbc_type == right.jdbc_type
    }

    /// 比较两个可空的原记录，返回相等布尔值或原映射输入异常。
    /// 对应 Java: DataCompareUtils#isRecordsEquals。
    pub fn java_records_equal(
        left: Option<&JavaCompareRecords<'_>>,
        right: Option<&JavaCompareRecords<'_>>,
        locale: &MetadataLocale,
    ) -> Result<bool, MetadataError> {
        let (Some(left), Some(right)) = (left, right) else {
            return Ok(left.is_none() && right.is_none());
        };
        if !Self::names_equal(&left.table_name, &right.table_name)
            || left.rows.map(<[_]>::len) != right.rows.map(<[_]>::len)
        {
            return Ok(false);
        }
        if left.rows.is_none_or(<[_]>::is_empty) {
            return Ok(true);
        }
        Self::java_rows_equal(left.primary_keys, left.rows, right.rows, locale)
    }

    /// 比较两个可空的可变记录，返回布尔结果或原输入／字段转换异常。
    /// 非空匹配记录使用左边的主键和显式序列化器，locale 决定列名折叠，时间解析由宿主提供。
    /// 对应 Java: DataCompareUtils#isRecordsEquals。
    pub fn java_records_equal_with_serializer(
        left: Option<JavaCompareRecordsMut<'_>>,
        right: Option<JavaCompareRecordsMut<'_>>,
        locale: &MetadataLocale,
        serializer: Option<&str>,
        parse_temporal: impl FnMut(
            JavaCompareTemporal,
            &JavaString,
        ) -> Result<JavaString, MetadataError>,
    ) -> Result<bool, MetadataError> {
        let (left, right) = match (left, right) {
            (None, None) => return Ok(true),
            (Some(left), Some(right)) => (left, right),
            _ => return Ok(false),
        };
        let name = left
            .table_name
            .as_ref()
            .ok_or(MetadataError::NullReference(
                "DataCompareUtils.beforeImage.tableName",
            ))?;
        if !right
            .table_name
            .as_ref()
            .is_some_and(|right| Self::names_equal(name, right))
            || left.rows.as_deref().map(<[_]>::len) != right.rows.as_deref().map(<[_]>::len)
        {
            return Ok(false);
        }
        // EmptyTableRecords 的元数据访问可以抛异常，原比较在空镜像时提前返回。
        if left.rows.as_deref().is_none_or(<[_]>::is_empty) {
            return Ok(true);
        }
        let primary_keys = left.primary_keys.ok_or(MetadataError::NullReference(
            "DataCompareUtils.beforeImage.tableMeta",
        ))?;
        Self::java_rows_equal_with_serializer(
            primary_keys,
            left.rows,
            right.rows,
            locale,
            serializer,
            parse_temporal,
        )
    }

    /// 按原字符串主键映射比较行，返回左侧所有字段是否在右侧匹配。
    /// 对应 Java: DataCompareUtils#isRowsEquals。
    pub fn java_rows_equal(
        primary_keys: &[JavaString],
        left: Option<&[Vec<JavaCompareField>]>,
        right: Option<&[Vec<JavaCompareField>]>,
        locale: &MetadataLocale,
    ) -> Result<bool, MetadataError> {
        if left.map(<[_]>::len) != right.map(<[_]>::len) {
            return Ok(false);
        }
        let (Some(left), Some(right)) = (left, right) else {
            return Err(MetadataError::NullReference("DataCompareUtils.rowList"));
        };
        let left = Self::java_row_list_to_map(left, primary_keys, locale)?;
        let right = Self::java_row_list_to_map(right, primary_keys, locale)?;
        Ok(left.iter().all(|(key, fields)| {
            right.get(key).is_some_and(|other| {
                fields.iter().all(|(name, field)| {
                    Self::java_fields_equal(Some(*field), other.get(name).copied())
                })
            })
        }))
    }

    /// 使用指定 Undo 序列化器比较可变行，返回比较结果或首个转换异常。
    /// 主键与列名映射在转换前固定，Locale 用于列名。Fastjson 时间由调用方的宿主解析。
    /// 对应 Java: DataCompareUtils#isRowsEquals 与当前 Undo serializer 的字段分派。
    pub fn java_rows_equal_with_serializer(
        primary_keys: &[JavaString],
        left: Option<&mut [Vec<JavaCompareField>]>,
        right: Option<&mut [Vec<JavaCompareField>]>,
        locale: &MetadataLocale,
        serializer: Option<&str>,
        mut parse_temporal: impl FnMut(
            JavaCompareTemporal,
            &JavaString,
        ) -> Result<JavaString, MetadataError>,
    ) -> Result<bool, MetadataError> {
        if serializer != Some("fastjson") {
            return Self::java_rows_equal(primary_keys, left.as_deref(), right.as_deref(), locale);
        }
        if left.as_ref().map(|rows| rows.len()) != right.as_ref().map(|rows| rows.len()) {
            return Ok(false);
        }
        let (Some(left), Some(right)) = (left, right) else {
            return Err(MetadataError::NullReference("DataCompareUtils.rowList"));
        };
        let left_index = Self::java_row_index(left, primary_keys, locale)?;
        let right_index = Self::java_row_index(right, primary_keys, locale)?;
        // 两侧行键均使用转换前的值，字段仍按原 HashMap 顺序更新到调用方的行。
        for key in left_index.keys() {
            let Some(old) = left_index.get(key.as_ref()) else {
                return Err(MetadataError::InvalidCompatibilityData(
                    "missing original row index",
                ));
            };
            let Some(new) = right_index.get(key.as_ref()) else {
                return Ok(false);
            };
            for name in old.1.keys() {
                let Some(old_field) = old.1.get(name.as_ref()) else {
                    return Err(MetadataError::InvalidCompatibilityData(
                        "missing original column index",
                    ));
                };
                let Some(new_field) = new.1.get(name.as_ref()) else {
                    return Ok(false);
                };
                if !Self::java_fastjson_fields_equal(
                    Some(&mut left[old.0][*old_field]),
                    Some(&mut right[new.0][*new_field]),
                    &mut parse_temporal,
                )? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn java_row_index(
        rows: &[Vec<JavaCompareField>],
        primary_keys: &[JavaString],
        locale: &MetadataLocale,
    ) -> Result<JavaRowIndex, MetadataError> {
        let mut index = JavaRowIndex::default();
        for (row_index, row) in rows.iter().enumerate() {
            let (key, columns) = Self::java_project_row(row, primary_keys, locale)?;
            let mut column_index = PrimaryKeyMap::default();
            for (name, field_index) in columns {
                column_index.insert(Some(name), Some(Arc::new(field_index)));
            }
            index.insert(Some(key), Some(Arc::new((row_index, column_index))));
        }
        Ok(index)
    }

    /// 对原行内列名按 UTF-16 排序，用原主键拼接、裁剪并按 Locale 大写列名。
    /// 对应 Java: DataCompareUtils#rowListToMap。
    pub fn java_row_list_to_map<'a>(
        rows: &'a [Vec<JavaCompareField>],
        primary_keys: &[JavaString],
        locale: &MetadataLocale,
    ) -> Result<JavaCompareRowMap<'a>, MetadataError> {
        let mut map = HashMap::with_capacity(rows.len());
        for row in rows {
            let (key, columns) = Self::java_project_row(row, primary_keys, locale)?;
            map.insert(
                key,
                columns
                    .into_iter()
                    .map(|(name, index)| (name, &row[index]))
                    .collect(),
            );
        }
        Ok(map)
    }

    fn java_project_row(
        row: &[JavaCompareField],
        primary_keys: &[JavaString],
        locale: &MetadataLocale,
    ) -> Result<(JavaString, Vec<(JavaString, usize)>), MetadataError> {
        let mut fields = row
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let name = field
                    .name
                    .as_ref()
                    .ok_or(MetadataError::NullReference("DataCompareUtils.fieldName"))?;
                Ok((name, index))
            })
            .collect::<Result<Vec<_>, MetadataError>>()?;
        fields.sort_by(|(left, _), (right, _)| left.as_utf16().cmp(right.as_utf16()));
        let mut columns = Vec::with_capacity(fields.len());
        let mut key = Vec::new();
        let mut first = true;
        for (name, index) in fields {
            if primary_keys.contains(name) {
                if !first {
                    key.push(u16::from(b'_'));
                }
                key.extend_from_slice(row[index].value.row_key()?.as_utf16());
                first = false;
            }
            columns.push((locale.upper(&name.java_trim())?, index));
        }
        Ok((JavaString::from_utf16(key), columns))
    }

    fn names_equal(left: &JavaString, right: &JavaString) -> bool {
        left.as_utf16().len() == right.as_utf16().len()
            && left.java_compare_to_ignore_case(right) == 0
    }

    /// 参数为同一物理元数据的两份完整镜像；忽略行序，要求两边主键均唯一，保留字段类型、值及 DECIMAL scale。
    /// 对应 Java: DataCompareUtils#isRecordsEquals/compareRows；不提供可空记录或未知JDBC类型的推断。
    pub(crate) fn records_equal(left: &TableRecords, right: &TableRecords) -> bool {
        if left.table_meta() != right.table_meta() || left.rows().len() != right.rows().len() {
            return false;
        }
        let mut map: HashMap<Vec<&FieldValue>, &Row> = left
            .rows()
            .iter()
            .map(|row| (row.primary_keys().map(|f| f.value()).collect(), row))
            .collect();
        map.len() == left.rows().len()
            && right.rows().iter().all(|row| {
                // 每个左侧主键只能匹配一次。右侧重复行也不能替代缺失行。
                map.remove(&row.primary_keys().map(|f| f.value()).collect::<Vec<_>>())
                    .is_some_and(|old| old == row)
            })
    }
}
