//! 固定原 Java 实际执行的上下文向量、原 Fence 码和 BMP 空白分类差分。
#![allow(clippy::unwrap_used)]
use base64::{engine::general_purpose::STANDARD, Engine};
use seata_tcc::{BusinessActionContext, CommonFenceStatus};
use std::collections::BTreeSet;

const VECTORS: &str = include_str!("fixtures/java_context.tsv");

#[test]
fn pinned_java_context_observations_match_rust() {
    let mut count = 0;
    for line in VECTORS.lines().skip(2) {
        let parts: Vec<_> = line.split('\t').collect();
        let token = parts.last().unwrap();
        let data = match *token {
            "-" => None,
            "~" => Some(String::new()),
            _ => Some(String::from_utf8(STANDARD.decode(token).unwrap()).unwrap()),
        };
        let result = BusinessActionContext::new(None, i64::MIN, None, data.as_deref());
        if parts[1] == "rejected" {
            assert!(result.is_err(), "original Java rejected {}", parts[0]);
        } else {
            let context = result.unwrap();
            assert_eq!(
                context.action_context().len(),
                parts[2].parse::<usize>().unwrap(),
                "{}",
                parts[0]
            );
            assert_eq!(
                context.use_common_fence(),
                parts[3].parse::<bool>().unwrap(),
                "{}",
                parts[0]
            );
            assert_eq!(context.branch_id(), parts[4].parse::<i64>().unwrap());
            assert_eq!(context.xid().is_none(), parts[5].parse::<bool>().unwrap());
            assert_eq!(
                context.action_name().is_none(),
                parts[6].parse::<bool>().unwrap()
            );
            if parts[7] != "-" {
                assert_eq!(context.get_action_context("x").unwrap().get(), parts[7]);
            }
        }
        count += 1;
    }
    assert_eq!(count, 21);
    let statuses: Vec<_> = VECTORS
        .lines()
        .next()
        .unwrap()
        .split('\t')
        .skip(1)
        .map(|v| v.parse::<i32>().unwrap())
        .collect();
    for (status, code) in [
        CommonFenceStatus::Tried,
        CommonFenceStatus::Committed,
        CommonFenceStatus::Rollbacked,
        CommonFenceStatus::Suspended,
    ]
    .into_iter()
    .zip(statuses)
    {
        assert_eq!(status.code(), code);
    }
}

#[test]
fn every_bmp_scalar_has_original_java_blank_classification() {
    let blank: BTreeSet<u32> = VECTORS
        .lines()
        .nth(1)
        .unwrap()
        .split('\t')
        .skip(1)
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(blank.len(), 25);
    for value in 0..=0xffff {
        if let Some(character) = char::from_u32(value) {
            let data = character.to_string();
            assert_eq!(
                BusinessActionContext::new(None, 0, None, Some(&data)).is_ok(),
                blank.contains(&value),
                "BMP {value:x}"
            );
        }
    }
}
