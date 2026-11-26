// Copyright 2026 Seata-Rust contributors.
// Licensed under the Apache License, Version 2.0.

use super::abstract_transaction_response::AbstractTransactionResponse;
use super::branch_report_response::BranchReportResponse;
use super::global_lock_query_response::GlobalLockQueryResponse;
use super::payload_message::PayloadMessage;
use super::result_code::ResultCode;
use super::transaction_exception_code::TransactionExceptionCode;
use std::collections::HashSet;
use std::path::PathBuf;

fn unhex(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn java_failed_text_boundaries_preserve_error_and_lockable_fields() {
    let directory = std::env::var_os("SEATA_IT_ERROR_LENGTH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/protocol/v1/error_lengths")
        });
    let rows = std::fs::read_to_string(directory.join("vectors.tsv")).unwrap();
    let mut seen = HashSet::new();
    for line in rows.lines() {
        let columns: Vec<_> = line.split('\t').collect();
        assert_eq!(columns.len(), 2);
        let name = columns[0];
        assert!(seen.insert(name.to_owned()));
        let (family, suffix) = name.split_once('_').unwrap();
        assert!(matches!(family, "Branch" | "Lock"));
        let text = match suffix {
            "Empty" | "Success" | "SuccessFalse" | "SuccessTrue" => String::new(),
            "AsciiOne" => "x".to_owned(),
            "Ascii127" => "x".repeat(127),
            "Ascii128" => "x".repeat(128),
            "Ascii255" => "x".repeat(255),
            "Ascii256" => "x".repeat(256),
            "Ascii32767" | "Ascii32768" => "x".repeat(32767),
            "Utf8Short" => "报错".repeat(22),
            "Utf8Boundary" => "错".repeat(10922),
            "FailedTrue" => "报错".to_owned(),
            _ => panic!("unknown Java error-length case: {name}"),
        };
        let success = matches!(suffix, "Success" | "SuccessFalse" | "SuccessTrue");
        let response = AbstractTransactionResponse {
            result_code: if success {
                ResultCode::Success
            } else {
                ResultCode::Failed
            },
            msg: if text.is_empty() { None } else { Some(text) },
            transaction_exception_code: if success {
                TransactionExceptionCode::Unknown
            } else if family == "Lock" {
                TransactionExceptionCode::LockableCheckFailed
            } else {
                TransactionExceptionCode::BranchReportFailed
            },
        };
        let expected = if family == "Lock" {
            PayloadMessage::GlobalLockQueryResponse(GlobalLockQueryResponse {
                response,
                lockable: matches!(suffix, "SuccessTrue" | "FailedTrue"),
            })
        } else {
            PayloadMessage::BranchReportResponse(BranchReportResponse { response })
        };
        let bytes = unhex(columns[1]);
        assert_eq!(PayloadMessage::decode(&bytes).unwrap(), expected, "{name}");
        assert_eq!(expected.encode().unwrap(), bytes, "{name}");
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(PayloadMessage::decode(&trailing).is_err(), "{name}");
    }
    for suffix in [
        "Empty",
        "AsciiOne",
        "Ascii127",
        "Ascii128",
        "Ascii255",
        "Ascii256",
        "Ascii32767",
        "Ascii32768",
        "Utf8Short",
        "Utf8Boundary",
    ] {
        for family in ["Branch", "Lock"] {
            assert!(seen.contains(&format!("{family}_{suffix}")));
        }
    }
    for name in [
        "Branch_Success",
        "Lock_SuccessFalse",
        "Lock_SuccessTrue",
        "Lock_FailedTrue",
    ] {
        assert!(seen.contains(name));
    }
    assert_eq!(seen.len(), 24);
}
