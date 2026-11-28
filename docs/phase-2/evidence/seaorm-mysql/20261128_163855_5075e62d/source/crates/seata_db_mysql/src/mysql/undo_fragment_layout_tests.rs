use super::undo_fragment_layout::UndoFragmentLayout;
use std::{collections::BTreeMap, error::Error, fmt::Write};

fn context_fields(context: &str) -> BTreeMap<&str, &str> {
    context
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .collect()
}

#[test]
fn original_oversized_log_has_java_fragments_and_exact_root_references(
) -> Result<(), Box<dyn Error>> {
    let java = include_str!("../../tests/fixtures/undo_fragment_layout_java.tsv");
    let (parts, root) = java
        .lines()
        .fold((Vec::new(), ""), |(mut parts, root), row| {
            if let Some(hex) = row.strip_prefix("PART\t") {
                parts.push(hex);
                (parts, root)
            } else {
                (parts, row.strip_prefix("ROOT\t").unwrap_or(root))
            }
        });
    let bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let mut layout = UndoFragmentLayout::new(42, 5, &bytes)?;
    assert_eq!(layout.child_count(), 2);
    layout.add_child(101)?;
    layout.add_child(102)?;
    let rows = layout.rows()?.collect::<Vec<_>>();
    assert_eq!(
        rows.iter().map(|row| row.0).collect::<Vec<_>>(),
        [101, 102, 42]
    );
    assert_eq!(rows[0].1, "branchId=42");
    assert_eq!(rows[1].1, "branchId=42");
    assert_eq!(
        rows.iter()
            .map(|row| row.2.iter().try_fold(String::new(), |mut hex, byte| {
                write!(hex, "{byte:02x}")?;
                Ok::<_, std::fmt::Error>(hex)
            }))
            .collect::<Result<Vec<_>, _>>()?,
        parts
    );
    let mut actual = context_fields(rows[2].1);
    // 生产 Rust 会明确写出压缩类型；同一 fixture 比对 Java 原例传入的全部原上下文字段。
    assert_eq!(actual.remove("compressorType"), Some("NONE"));
    assert_eq!(actual, context_fields(root));
    Ok(())
}

#[test]
fn layout_withholds_rows_until_every_child_identity_is_known_and_valid(
) -> Result<(), Box<dyn Error>> {
    let bytes = [1; 12];
    let mut layout = UndoFragmentLayout::new(42, 5, &bytes)?;
    assert!(layout.rows().is_err());
    assert!(layout.add_child(42).is_err());
    assert!(layout.add_child(0).is_err());
    layout.add_child(101)?;
    assert!(layout.add_child(101).is_err());
    assert!(layout.rows().is_err());
    layout.add_child(9)?;
    assert!(layout.add_child(7).is_err());
    let rows = layout.rows()?.collect::<Vec<_>>();
    assert_eq!(
        rows[2].1,
        "serializer=jackson&compressorType=NONE&map=5&subId=101,9,"
    );
    Ok(())
}

#[test]
fn layout_preserves_exact_packet_boundary_and_rejects_empty_or_unrepresentable_payloads(
) -> Result<(), Box<dyn Error>> {
    let exactly = [0; 819];
    let result = UndoFragmentLayout::new(42, 1024, &exactly)?;
    assert_eq!(result.child_count(), 0);
    assert_eq!(
        result.rows()?.collect::<Vec<_>>(),
        [(
            42,
            "serializer=jackson&compressorType=NONE&map=1024",
            exactly.as_slice()
        )]
    );
    assert_eq!(
        UndoFragmentLayout::new(42, 1024, &[0; 820])?.child_count(),
        1
    );
    assert!(UndoFragmentLayout::new(42, 1, &exactly).is_err());
    assert!(UndoFragmentLayout::new(42, 1024, &[]).is_err());
    assert!(UndoFragmentLayout::new(42, 5, &[0; 1024]).is_err());

    let bytes = [0; 24];
    let mut result = UndoFragmentLayout::new(42, 5, &bytes)?;
    for id in (i64::MAX - 2)..=i64::MAX {
        result.add_child(id)?;
    }
    assert!(result.add_child(i64::MAX - 3).is_err());
    assert!(result.rows().is_err());

    let mut exact_context = UndoFragmentLayout::new(42, 5, &[0; 20])?;
    for id in (i64::MAX - 2)..=i64::MAX {
        exact_context.add_child(id)?;
    }
    exact_context.add_child(1_000_000_000_000_000)?;
    let rows = exact_context.rows()?.collect::<Vec<_>>();
    assert_eq!(rows[4].1.len(), 128);
    Ok(())
}
