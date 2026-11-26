use crate::{EscapeHandlerFactory, JavaString, MetadataError, TableMeta};

/// 通过数据库方言对单个或多个列名添加、删除转义。
/// 对应 Java: org.apache.seata.sqlparser.util.ColumnUtils。
pub struct ColumnUtils;

impl ColumnUtils {
    /// 对可空 col_name 添加 db_type 对应的转义；空白输入直接返回。
    /// table 用于方言回调，factory 提供数据库处理器。
    /// 对应 Java: ColumnUtils#addEscape(String, String, TableMeta)。
    pub fn add_escape(
        col_name: Option<JavaString>,
        db_type: Option<&JavaString>,
        table: Option<&TableMeta>,
        factory: &EscapeHandlerFactory,
    ) -> Result<Option<JavaString>, MetadataError> {
        if col_name.as_ref().is_none_or(JavaString::is_java_blank) {
            return Ok(col_name);
        }
        factory
            .get_escape_handler(db_type)?
            .add_col_name_escape_with_table(col_name.as_ref(), table)
            .map(Some)
    }

    /// 对可空 cols 按原列表顺序逐项添加转义，空列表不查找方言。
    /// 对应 Java: ColumnUtils#addEscape(List, String)；factory 为 Rust 注入的服务域。
    pub fn add_escape_list(
        cols: Option<Vec<Option<JavaString>>>,
        db_type: Option<&JavaString>,
        factory: &EscapeHandlerFactory,
    ) -> Result<Option<Vec<Option<JavaString>>>, MetadataError> {
        cols.map(|cols| Self::add_escapes(cols, db_type, factory))
            .transpose()
    }

    /// 处理非空引用的列名列表，供 TableMeta 的主键路径复用。
    /// 对应 Java: ColumnUtils#addEscape(List, String) 的非空引用路径。
    pub(crate) fn add_escapes(
        cols: Vec<Option<JavaString>>,
        db_type: Option<&JavaString>,
        factory: &EscapeHandlerFactory,
    ) -> Result<Vec<Option<JavaString>>, MetadataError> {
        cols.into_iter()
            .map(|col| Self::add_escape(col, db_type, None, factory))
            .collect()
    }

    /// 用 factory 先选择 db_type 的处理器，再删除可空 col_name 的转义。
    /// 对应 Java: ColumnUtils#delEscape(String, String)。
    pub fn del_escape(
        col_name: Option<JavaString>,
        db_type: Option<&JavaString>,
        factory: &EscapeHandlerFactory,
    ) -> Result<Option<JavaString>, MetadataError> {
        factory
            .get_escape_handler(db_type)?
            .del_col_name_escape(col_name.as_ref())
    }

    /// 对可空 cols 按原列表顺序逐项删除转义，保留 null 元素。
    /// 对应 Java: ColumnUtils#delEscape(List, String)；factory 为 Rust 注入的服务域。
    pub fn del_escape_list(
        cols: Option<Vec<Option<JavaString>>>,
        db_type: Option<&JavaString>,
        factory: &EscapeHandlerFactory,
    ) -> Result<Option<Vec<Option<JavaString>>>, MetadataError> {
        cols.map(|cols| {
            cols.into_iter()
                .map(|col| Self::del_escape(col, db_type, factory))
                .collect()
        })
        .transpose()
    }
}
