use crate::DbError;

/// 由原 Normal Undo 根行上下文确认的序列化器。
/// 对应 Java: AbstractUndoLogManager#undo 中的解析器选择。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UndoSerializer {
    Jackson,
    Fastjson,
    Fastjson2,
}

impl UndoSerializer {
    pub(crate) fn parse_context(name: Option<&str>) -> Result<Self, DbError> {
        match name {
            Some("jackson") => Ok(Self::Jackson),
            Some("fastjson") => Ok(Self::Fastjson),
            Some("fastjson2") => Ok(Self::Fastjson2),
            _ => Err(Self::unsupported()),
        }
    }

    pub(crate) fn require_context(
        self,
        serializer: Option<&str>,
        compressor: Option<&str>,
    ) -> Result<(), DbError> {
        if Self::parse_context(serializer)? == self && compressor == Some("NONE") {
            Ok(())
        } else {
            Err(Self::unsupported())
        }
    }

    fn unsupported() -> DbError {
        DbError::Unsupported {
            feature: "Undo supported serializer/compressor",
        }
    }
}
