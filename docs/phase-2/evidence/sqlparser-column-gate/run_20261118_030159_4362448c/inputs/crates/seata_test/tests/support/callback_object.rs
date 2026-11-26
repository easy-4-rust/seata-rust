use seata_sqlparser::{ColumnMeta, JavaObject, JavaString};
use std::any::Any;

/// 测试夹具：区分原Object的三种可重入回调。
#[derive(Debug, Clone, Copy)]
pub enum CallbackKind {
    Equals,
    Hash,
    Text,
}

/// 测试夹具：按原Java观察器相同顺序修改共享列对象。
#[derive(Debug)]
pub struct CallbackObject {
    pub column: ColumnMeta,
    pub kind: CallbackKind,
}

impl JavaObject for CallbackObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, other: &dyn JavaObject) -> bool {
        if matches!(self.kind, CallbackKind::Equals) {
            assert!(self.column.set_ordinal_position(7).is_ok());
        }
        other.as_any().is::<Self>()
    }
    fn java_hash_code(&self) -> i32 {
        if matches!(self.kind, CallbackKind::Hash) {
            assert!(self.column.set_ordinal_position(7).is_ok());
        }
        11
    }
    fn java_to_string(&self) -> Option<JavaString> {
        if matches!(self.kind, CallbackKind::Text) {
            assert!(self.column.set_ordinal_position(7).is_ok());
            assert!(self
                .column
                .set_is_null_able(Some("callback null".into()))
                .is_ok());
            assert!(self.column.set_is_autoincrement(Some("YES".into())).is_ok());
            assert!(self.column.set_on_update(true).is_ok());
            assert!(self.column.set_case_sensitive(true).is_ok());
        }
        Some("callback".into())
    }
}
