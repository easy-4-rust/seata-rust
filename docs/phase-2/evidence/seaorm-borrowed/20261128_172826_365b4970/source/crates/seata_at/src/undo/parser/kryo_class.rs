use super::jackson_undo_log_parser::{EMPTY, FIELD, RECORDS, ROW, SQL};

/// 固定 Kryo 5.4 工厂的原 Undo 类词汇，不加载运行时类。
/// 对应 Java: UndoLogSerializerClassRegistry 与 KryoSerializerFactory。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum KryoClass {
    Branch,
    ArrayList,
    Sql,
    Records,
    EmptyRecords,
    Row,
    Field,
    Integer,
    Long,
    String,
    Boolean,
    Double,
    Timestamp,
}

impl KryoClass {
    pub(super) const fn registered(self) -> Option<u32> {
        match self {
            Self::Integer => Some(2),
            Self::String => Some(3),
            Self::Boolean => Some(5),
            Self::Long => Some(9),
            Self::Double => Some(10),
            Self::Timestamp => Some(13),
            Self::ArrayList => Some(24),
            Self::Branch => Some(44),
            _ => None,
        }
    }

    pub(super) fn by_token(token: u32) -> Option<Self> {
        [
            Self::Integer,
            Self::String,
            Self::Boolean,
            Self::Long,
            Self::Double,
            Self::Timestamp,
            Self::ArrayList,
            Self::Branch,
        ]
        .iter()
        .copied()
        .find(|class| class.registered() == Some(token))
    }

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Sql => SQL,
            Self::Records => RECORDS,
            Self::EmptyRecords => EMPTY,
            Self::Row => ROW,
            Self::Field => FIELD,
            _ => "",
        }
    }

    pub(super) fn by_name(name: &str) -> Option<Self> {
        [
            Self::Sql,
            Self::Records,
            Self::EmptyRecords,
            Self::Row,
            Self::Field,
        ]
        .iter()
        .copied()
        .find(|class| class.name() == name)
    }
}
