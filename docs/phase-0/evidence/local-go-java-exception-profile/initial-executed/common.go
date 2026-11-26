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
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	e "seata.apache.org/seata-go/v2/pkg/util/errors"
)

type row struct {
	Name      string `json:"name"`
	Type      int    `json:"type_id"`
	Code      int    `json:"error_code"`
	ErrorName string `json:"error_name"`
	Hex       string `json:"serialized_hex"`
}

var names = []string{"Unknown", "BeginFailed", "LockKeyConflict", "IO", "BranchRollbackFailed_Retriable", "BranchRollbackFailed_Unretriable", "BranchRegisterFailed", "BranchReportFailed", "LockableCheckFailed", "BranchTransactionNotExist", "GlobalTransactionNotExist", "GlobalTransactionNotActive", "GlobalTransactionStatusInvalid", "FailedToSendBranchCommitRequest", "FailedToSendBranchRollbackRequest", "FailedToAddBranch", "FailedLockGlobalTransaction", "FailedWriteSession", "FailedStore", "NotRaftLeader", "LockKeyConflictFailFast", "TransactionTimeout", "CommitHeuristic", "Broken", "BranchPrepareFailed"}

// New Java-only domain values are outside legacy byte-sized wire enums.
var domain = []int32{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 256, 19, 257, 258, 259, 260}

func sample(kind, code int) message.MessageTypeAware {
	base := message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeFailed, Msg: "wire:" + names[code]}, TransactionErrorCode: e.TransactionErrorCode(domain[code])}
	end := message.AbstractGlobalEndResponse{AbstractTransactionResponse: base, GlobalStatus: message.GlobalStatusRollbackRetrying}
	branchEnd := message.AbstractBranchEndResponse{AbstractTransactionResponse: base, Xid: "127.0.0.1:8091:33", BranchId: 0x0102030405060708, BranchStatus: branch.BranchStatusPhasetwoCommitFailedRetryable}
	switch kind {
	case 2:
		return message.GlobalBeginResponse{AbstractTransactionResponse: base, Xid: "127.0.0.1:8091:33", ExtraData: []byte("meta")}
	case 4:
		return message.BranchCommitResponse{AbstractBranchEndResponse: branchEnd}
	case 6:
		branchEnd.BranchStatus = branch.BranchStatusPhasetwoRollbackFailedRetryable
		return message.BranchRollbackResponse{AbstractBranchEndResponse: branchEnd}
	case 8:
		return message.GlobalCommitResponse{AbstractGlobalEndResponse: end}
	case 10:
		return message.GlobalRollbackResponse{AbstractGlobalEndResponse: end}
	case 12:
		return message.BranchRegisterResponse{AbstractTransactionResponse: base, BranchId: 0x0102030405060708}
	case 14:
		return message.BranchReportResponse{AbstractTransactionResponse: base}
	case 16:
		return message.GlobalStatusResponse{AbstractGlobalEndResponse: end}
	case 18:
		return message.GlobalReportResponse{AbstractGlobalEndResponse: end}
	case 22:
		return message.GlobalLockQueryResponse{AbstractTransactionResponse: base, Lockable: false}
	default:
		panic("unexpected fixture kind")
	}
}

func verify(encode func(interface{}) ([]byte, error), decode func([]byte) (interface{}, error)) {
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	var rows []row
	if err = json.Unmarshal(data, &rows); err != nil {
		panic(err)
	}
	if len(rows) != 250 {
		panic("need250 actual Java rows")
	}
	seen := map[string]bool{}
	pass, fail := 0, 0
	for _, r := range rows {
		if r.Code < 0 || r.Code >= 25 || r.ErrorName != names[r.Code] || r.Name != fmt.Sprintf("%d_%s", r.Type, names[r.Code]) || seen[r.Name] {
			panic("invalid/duplicate case")
		}
		seen[r.Name] = true
		want := sample(r.Type, r.Code)
		wire, err := hex.DecodeString(r.Hex)
		if err != nil {
			panic(err)
		}
		encoded, encErr := encode(want)
		decoded, decErr := decode(wire)
		encodeOK := encErr == nil && bytes.Equal(encoded, wire)
		decodeOK := decErr == nil && reflect.DeepEqual(decoded, want)
		roundOK := false
		if decodeOK {
			back, err := encode(decoded)
			roundOK = err == nil && bytes.Equal(back, wire)
		}
		if encodeOK && decodeOK && roundOK {
			pass++
		} else {
			fail++
		}
		result, _ := json.Marshal(map[string]interface{}{"name": r.Name, "encode_match": encodeOK, "decode_match": decodeOK, "reencode_match": roundOK})
		fmt.Println("SEATA_EXCEPTION_RESULT " + string(result))
	}
	for _, kind := range []int{2, 4, 6, 8, 10, 12, 14, 16, 18, 22} {
		for _, name := range names {
			if !seen[fmt.Sprintf("%d_%s", kind, name)] {
				panic("missing case")
			}
		}
	}
	fmt.Printf("EXCEPTION SUMMARY matched=%d divergent=%d\n", pass, fail)
	if fail > 0 {
		os.Exit(1)
	}
}
