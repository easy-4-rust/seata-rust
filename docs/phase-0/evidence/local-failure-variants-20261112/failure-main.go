// Copyright 2026 Seata-Rust contributors.
// Licensed under the Apache License, Version 2.0.
package main

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"reflect"

	"seata.apache.org/seata-go/v2/pkg/protocol/branch"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	seataerrors "seata.apache.org/seata-go/v2/pkg/util/errors"
)

type failureVector struct {
	Name              string `json:"name"`
	TypeID            int    `json:"type_id"`
	Compatibility     string `json:"compatibility"`
	GoBodyHex         string `json:"go_body_hex"`
	GoSerializedHex   string `json:"go_serialized_hex"`
	JavaSerializedHex string `json:"java_serialized_hex"`
}

func failed(code seataerrors.TransactionErrorCode, msg string) message.AbstractTransactionResponse {
	return message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeFailed, Msg: msg}, TransactionErrorCode: code}
}

func main() {
	// 独立类型和字段预期，不从待比较的 JSON 构建语义。
	samples := []struct {
		name   string
		value  message.MessageTypeAware
		status string
	}{
		{"BranchRollbackResponseFailed", message.BranchRollbackResponse{AbstractBranchEndResponse: message.AbstractBranchEndResponse{AbstractTransactionResponse: failed(seataerrors.TransactionErrorCodeBranchRollbackFailedRetriable, "rollback retry"), Xid: "127.0.0.1:8091:1", BranchId: 0x0102030405060708, BranchStatus: branch.BranchStatusPhasetwoRollbackFailedRetryable}}, "byte_equal"},
		{"GlobalRollbackResponseFailed", message.GlobalRollbackResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: failed(seataerrors.TransactionErrorCodeIO, "rollback pending"), GlobalStatus: message.GlobalStatusRollbackRetrying}}, "byte_equal"},
		{"BranchReportResponseFailed", message.BranchReportResponse{AbstractTransactionResponse: failed(seataerrors.TransactionErrorCodeBranchReportFailed, "report denied")}, "error_text_length"},
		{"GlobalStatusResponseFailed", message.GlobalStatusResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: failed(seataerrors.TransactionErrorCodeIO, "status unavailable"), GlobalStatus: message.GlobalStatusUnKnown}}, "byte_equal"},
		{"GlobalReportResponseFailed", message.GlobalReportResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: failed(seataerrors.TransactionErrorCodeIO, "report pending"), GlobalStatus: message.GlobalStatusRollbacking}}, "byte_equal"},
		{"GlobalLockQueryResponseFailed", message.GlobalLockQueryResponse{AbstractTransactionResponse: failed(seataerrors.TransactionErrorCodeLockableCheckFailed, "lock check failed"), Lockable: false}, "error_text_length"},
		{"BranchRegisterLockKeyConflictFailFast", message.BranchRegisterResponse{AbstractTransactionResponse: failed(seataerrors.LockKeyConflictFailFast, "fail fast"), BranchId: 0}, "exception_code_ordinal"},
	}
	raw, err := os.ReadFile("expected.json")
	must(err)
	var vectors []failureVector
	must(json.Unmarshal(raw, &vectors))
	require(len(vectors) == 7 && len(vectors) == len(samples), "all seven failure cases required")
	codec.Init()
	manager := codec.GetCodecManager()
	equal, divergent := 0, 0
	for at, sample := range samples {
		row := vectors[at]
		require(row.Name == sample.name && row.TypeID == int(sample.value.GetTypeCode()), "wrong case identity")
		require(row.Compatibility == sample.status, "wrong compatibility classification")
		native := manager.GetCodec(codec.CodecTypeSeata, sample.value.GetTypeCode())
		require(native != nil, "codec missing")
		body := native.Encode(sample.value)
		encoded := manager.Encode(codec.CodecTypeSeata, sample.value)
		require(bytes.Equal(body, unhex(row.GoBodyHex)), sample.name+": wrong Go body")
		require(bytes.Equal(encoded, unhex(row.GoSerializedHex)), sample.name+": wrong Go serialized bytes")
		decoded := manager.Decode(codec.CodecTypeSeata, encoded)
		require(reflect.DeepEqual(decoded, sample.value), sample.name+": wrong typed Go semantics")
		require(bytes.Equal(manager.Encode(codec.CodecTypeSeata, decoded), encoded), sample.name+": unstable Go roundtrip")
		matchesJava := bytes.Equal(encoded, unhex(row.JavaSerializedHex))
		require(matchesJava == (sample.status == "byte_equal"), sample.name+": unexpected cross-language result")
		if matchesJava {
			equal++
			fmt.Printf("COMPATIBLE %s type=%d bytes=%x\n", sample.name, row.TypeID, encoded)
		} else {
			divergent++
			fmt.Printf("DIVERGENCE %s type=%d reason=%s Go=%x Java=%s\n", sample.name, row.TypeID, sample.status, encoded, row.JavaSerializedHex)
		}
	}
	require(equal == 4 && divergent == 3, "missing compatibility outcomes")
	fmt.Println("DIAGNOSTIC PASS: 7 Go-native roundtrips; 4 byte-compatible, 3 OPEN divergences. This is not full interoperability certification.")
}

func unhex(value string) []byte { out, err := hex.DecodeString(value); must(err); return out }
func must(err error) {
	if err != nil {
		panic(err)
	}
}
func require(ok bool, text string) {
	if !ok {
		fmt.Fprintln(os.Stderr, text)
		os.Exit(1)
	}
}
