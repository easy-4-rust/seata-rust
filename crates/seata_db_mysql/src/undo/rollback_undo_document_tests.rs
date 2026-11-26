#![allow(clippy::unwrap_used)]
use super::{RollbackUndoDocument, UndoSerializer};
use crate::DbError;

const WIRE: &[u8] = include_bytes!("../../../seata_at/tests/fixtures/fastjson_native_wire.txt");
const XID: &str = "fastjson-native:42";

#[test]
fn locked_serializer_selects_exact_decoder_before_table_lookup() {
    assert_eq!(
        UndoSerializer::parse_context(Some("fastjson")).unwrap(),
        UndoSerializer::Fastjson
    );
    assert_eq!(
        UndoSerializer::parse_context(Some("jackson")).unwrap(),
        UndoSerializer::Jackson
    );
    assert!(UndoSerializer::parse_context(Some("Fastjson")).is_err());
    assert!(UndoSerializer::parse_context(None).is_err());

    assert!(UndoSerializer::Jackson
        .require_context(Some("fastjson"), Some("NONE"))
        .is_err());
    assert!(UndoSerializer::Fastjson
        .require_context(Some("jackson"), Some("NONE"))
        .is_err());
    assert!(UndoSerializer::Fastjson
        .require_context(Some("fastjson"), Some("ZIP"))
        .is_err());
    let document = RollbackUndoDocument::decode(UndoSerializer::Fastjson, WIRE, XID, 42).unwrap();
    assert_eq!(document.table_names().unwrap(), ["undo_fixture"]);
    assert!(RollbackUndoDocument::decode(UndoSerializer::Fastjson, WIRE, "other", 42).is_err());
    assert!(RollbackUndoDocument::decode(UndoSerializer::Jackson, WIRE, XID, 42).is_err());
    assert!(matches!(
        RollbackUndoDocument::decode(UndoSerializer::Jackson, b"null", XID, 42),
        Err(DbError::Input {
            reason: "Undo non-null original document"
        })
    ));
}

const JSONB: &[u8] = include_bytes!("../../../seata_at/tests/fixtures/fastjson2_native_wire.bin");

#[test]
fn fastjson2_requires_exact_locked_context_and_preflights_jsonb_before_table_lookup() {
    let format = UndoSerializer::parse_context(Some("fastjson2")).unwrap();
    assert_eq!(format, UndoSerializer::Fastjson2);
    assert!(format
        .require_context(Some("fastjson2"), Some("NONE"))
        .is_ok());
    for (serializer, compressor) in [
        (Some("fastjson"), Some("NONE")),
        (Some("Fastjson2"), Some("NONE")),
        (Some("fastjson2"), Some("ZIP")),
        (Some("fastjson2"), None),
    ] {
        assert!(format.require_context(serializer, compressor).is_err());
    }
    assert!(UndoSerializer::Jackson
        .require_context(Some("fastjson2"), Some("NONE"))
        .is_err());
    assert!(UndoSerializer::Fastjson
        .require_context(Some("fastjson2"), Some("NONE"))
        .is_err());

    let document = RollbackUndoDocument::decode(format, JSONB, XID, 42).unwrap();
    assert_eq!(document.table_names().unwrap(), ["undo_fixture"]);
    assert!(RollbackUndoDocument::decode(format, JSONB, XID, 43).is_err());
    assert!(RollbackUndoDocument::decode(format, WIRE, XID, 42).is_err());
    assert!(RollbackUndoDocument::decode(UndoSerializer::Fastjson, JSONB, XID, 42).is_err());

    // 改坏第一条镜像的表名，必须在拿到任何表名供 SQL 查询前拒绝。
    let mut bad = JSONB.to_vec();
    let table = b"undo_fixture";
    let offset = bad
        .windows(table.len())
        .position(|window| window == table)
        .unwrap();
    bad[offset] = b'x';
    assert!(RollbackUndoDocument::decode(format, &bad, XID, 42).is_err());
}
