// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
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
	e "seata.apache.org/seata-go/v2/pkg/util/errors"
)

type fixture struct {
	Name    string `json:"name"`
	Type    int    `json:"type_id"`
	Hex     string `json:"serialized_hex"`
	JavaHex string `json:"java_serialized_hex"`
}

func require(ok bool) {
	if !ok {
		panic("profile corpus regression failed")
	}
}
func load(path string) []fixture {
	raw, err := os.ReadFile(path)
	if err != nil {
		panic(err)
	}
	var rows []fixture
	err = json.Unmarshal(raw, &rows)
	if err != nil {
		panic(err)
	}
	return rows
}
func failure(name string) interface{} {
	base := func(code e.TransactionErrorCode, msg string) message.AbstractTransactionResponse {
		return message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeFailed, Msg: msg}, TransactionErrorCode: code}
	}
	switch name {
	case "BranchRollbackResponseFailed":
		return message.BranchRollbackResponse{AbstractBranchEndResponse: message.AbstractBranchEndResponse{AbstractTransactionResponse: base(e.TransactionErrorCodeBranchRollbackFailedRetriable, "rollback retry"), Xid: "127.0.0.1:8091:1", BranchId: 0x0102030405060708, BranchStatus: branch.BranchStatusPhasetwoRollbackFailedRetryable}}
	case "GlobalRollbackResponseFailed":
		return message.GlobalRollbackResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: base(e.TransactionErrorCodeIO, "rollback pending"), GlobalStatus: message.GlobalStatusRollbackRetrying}}
	case "BranchReportResponseFailed":
		return message.BranchReportResponse{AbstractTransactionResponse: base(e.TransactionErrorCodeBranchReportFailed, "report denied")}
	case "GlobalStatusResponseFailed":
		return message.GlobalStatusResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: base(e.TransactionErrorCodeIO, "status unavailable"), GlobalStatus: message.GlobalStatusUnKnown}}
	case "GlobalReportResponseFailed":
		return message.GlobalReportResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: base(e.TransactionErrorCodeIO, "report pending"), GlobalStatus: message.GlobalStatusRollbacking}}
	case "GlobalLockQueryResponseFailed":
		return message.GlobalLockQueryResponse{AbstractTransactionResponse: base(e.TransactionErrorCodeLockableCheckFailed, "lock check failed"), Lockable: false}
	case "BranchRegisterLockKeyConflictFailFast":
		return message.BranchRegisterResponse{AbstractTransactionResponse: base(e.LockKeyConflictFailFast, "fail fast"), BranchId: 0}
	default:
		panic("unknown failure fixture")
	}
}

func main() {
	codec.Init()
	native := codec.GetCodecManager()
	p, err := codec.NewJavaV1ResponseProfile(native)
	if err != nil {
		panic(err)
	}
	ordinary := load(os.Args[1])
	require(len(ordinary) == 29)
	supported, rejected := 0, 0
	for _, r := range ordinary {
		wire, err := hex.DecodeString(r.Hex)
		if err != nil {
			panic(err)
		}
		value, err := p.Decode(wire)
		good := r.Type == 2 || r.Type == 4 || r.Type == 6 || r.Type == 8 || r.Type == 10 || r.Type == 12 || r.Type == 14 || r.Type == 16 || r.Type == 18 || r.Type == 22
		if !good {
			require(err != nil && value == nil)
			rejected++
			continue
		}
		require(err == nil && reflect.DeepEqual(value, native.Decode(codec.CodecTypeSeata, wire)))
		encoded, err := p.Encode(value)
		require(err == nil && bytes.Equal(encoded, wire))
		supported++
	}
	require(supported == 14 && rejected == 15)
	failed := load(os.Args[2])
	require(len(failed) == 7)
	seen := map[string]bool{}
	for _, r := range failed {
		require(!seen[r.Name])
		seen[r.Name] = true
		want := failure(r.Name)
		wire, err := hex.DecodeString(r.JavaHex)
		if err != nil {
			panic(err)
		}
		encoded, err := p.Encode(want)
		require(err == nil && bytes.Equal(encoded, wire))
		value, err := p.Decode(wire)
		require(err == nil && reflect.DeepEqual(value, want))
	}
	lengths := load(os.Args[3])
	require(len(lengths) == 24)
	for _, r := range lengths {
		wire, err := hex.DecodeString(r.Hex)
		if err != nil {
			panic(err)
		}
		value, err := p.Decode(wire)
		require(err == nil)
		encoded, err := p.Encode(value)
		require(err == nil && bytes.Equal(encoded, wire))
	}
	fmt.Println("PROFILE CORPUS PASS: original14 response bytes,15 non-response rejections,all7 Java failure semantics,24 length wire roundtrips")
}
