// Copyright 2026 Seata-Rust contributors.
// Licensed under the Apache License, Version 2.0.
package main

import (
	"bytes"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"strings"
	"unicode/utf8"

	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	seataerrors "seata.apache.org/seata-go/v2/pkg/util/errors"
)

type expectation struct {
	operation         string
	result, errorCode int
	msg               string
	status            int
	xid               string
	branchID          int64
	lockable          bool
	version           string
}

func expectedNames() map[string]bool {
	names := map[string]bool{}
	for _, op := range []string{"begin", "commit", "rollback", "status", "report", "register", "branch_report", "lock"} {
		for _, failure := range []string{"none", "transaction", "runtime", "store"} {
			names["matrix_"+op+"_"+failure] = true
		}
	}
	for _, op := range []string{"commit", "rollback", "status"} {
		for _, failure := range []string{"transaction", "runtime", "store"} {
			for _, lookup := range []string{"absent", "throws", "getter_throws", "mutated_then_throws"} {
				names["fallback_"+op+"_"+failure+"_"+lookup] = true
			}
		}
	}
	for _, name := range []string{"conflict_lockkeyconflict", "conflict_lockkeyconflictfailfast", "lock_success_false"} {
		names[name] = true
	}
	for _, role := range []string{"tm", "rm"} {
		for _, auth := range []string{"allow", "deny", "throw"} {
			names["auth_"+role+"_"+auth] = true
		}
	}
	names["auth_rm_deny_closed"] = true
	require(len(names) == 78, "expected78 case identities")
	return names
}

func integer(value string) int { out, err := strconv.Atoi(value); must(err); return out }
func text(value string) string {
	raw, err := hex.DecodeString(value)
	must(err)
	require(utf8.Valid(raw), "expectation must be valid UTF-8")
	return string(raw)
}
func read(path string) []byte { raw, err := os.ReadFile(path); must(err); return raw }
func parse(raw []byte) expectation {
	parts := strings.Split(strings.TrimRight(string(raw), "\r\n"), "\t")
	require(len(parts) == 9, "expectation must contain9 columns")
	branch, err := strconv.ParseInt(parts[6], 10, 64)
	must(err)
	flag, lock := integer(parts[1]), integer(parts[7])
	require((flag == 0 || flag == 1) && (lock == 0 || lock == 1), "invalid boolean/result")
	return expectation{parts[0], flag, integer(parts[2]), text(parts[3]), integer(parts[4]), text(parts[5]), branch, lock == 1, text(parts[8])}
}

func nativeError(code int) seataerrors.TransactionErrorCode {
	// 显式同义映射；不把 Java20 的字节当作 Go20 的同名错误。
	switch code {
	case 0:
		return seataerrors.TransactionErrorCodeUnknown
	case 1:
		return seataerrors.TransactionErrorCodeBeginFailed
	case 2:
		return seataerrors.TransactionErrorCodeLockKeyConflict
	case 3:
		return seataerrors.TransactionErrorCodeIO
	case 6:
		return seataerrors.TransactionErrorCodeBranchRegisterFailed
	case 7:
		return seataerrors.TransactionErrorCodeBranchReportFailed
	case 8:
		return seataerrors.TransactionErrorCodeLockableCheckFailed
	case 18:
		return seataerrors.FailedStore
	case 20:
		return seataerrors.LockKeyConflictFailFast
	default:
		panic(fmt.Sprintf("No certified semantic Go mapping for Java error%d", code))
	}
}

func model(e expectation) message.MessageTypeAware {
	result := message.ResultCodeFailed
	if e.result == 1 {
		result = message.ResultCodeSuccess
	}
	tx := message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: result, Msg: e.msg}, TransactionErrorCode: nativeError(e.errorCode)}
	global := message.AbstractGlobalEndResponse{AbstractTransactionResponse: tx, GlobalStatus: message.GlobalStatus(e.status)}
	identify := message.AbstractIdentifyResponse{Identified: e.result == 1, Version: e.version}
	switch e.operation {
	case "BEGIN":
		return message.GlobalBeginResponse{AbstractTransactionResponse: tx, Xid: e.xid, ExtraData: []byte{}}
	case "COMMIT":
		return message.GlobalCommitResponse{AbstractGlobalEndResponse: global}
	case "ROLLBACK":
		return message.GlobalRollbackResponse{AbstractGlobalEndResponse: global}
	case "STATUS":
		return message.GlobalStatusResponse{AbstractGlobalEndResponse: global}
	case "REPORT":
		return message.GlobalReportResponse{AbstractGlobalEndResponse: global}
	case "REGISTER":
		return message.BranchRegisterResponse{AbstractTransactionResponse: tx, BranchId: e.branchID}
	case "BRANCH_REPORT":
		return message.BranchReportResponse{AbstractTransactionResponse: tx}
	case "LOCK":
		return message.GlobalLockQueryResponse{AbstractTransactionResponse: tx, Lockable: e.lockable}
	case "REG_TM":
		return message.RegisterTMResponse{AbstractIdentifyResponse: identify}
	case "REG_RM":
		return message.RegisterRMResponse{AbstractIdentifyResponse: identify}
	default:
		panic("unsupported handler operation " + e.operation)
	}
}

func safeDecode(manager *codec.CodecManager, payload []byte) (value interface{}, panicked bool) {
	defer func() {
		if recover() != nil {
			value = nil
			panicked = true
		}
	}()
	return manager.Decode(codec.CodecTypeSeata, payload), false
}

func main() {
	require(len(os.Args) == 2, "usage: go_probe <captured-handler-fixture-directory>")
	dir := os.Args[1]
	known := expectedNames()
	for _, ext := range []string{".expect", ".bin"} {
		files, err := filepath.Glob(filepath.Join(dir, "*"+ext))
		must(err)
		require(len(files) == len(known), "all78 cases required for "+ext)
		for _, path := range files {
			require(known[strings.TrimSuffix(filepath.Base(path), ext)], "unknown case artifact "+path)
		}
	}
	codec.Init()
	manager := codec.GetCodecManager()
	compatible, lengthDiff, ordinalDiff, opaque, truncated := 0, 0, 0, 0, 0
	files, err := filepath.Glob(filepath.Join(dir, "*.expect"))
	must(err)
	for _, path := range files {
		name := strings.TrimSuffix(filepath.Base(path), ".expect")
		e := parse(read(path))
		expected := model(e)
		java := read(filepath.Join(dir, name+".bin"))
		goBytes := manager.Encode(codec.CodecTypeSeata, expected)
		nativeExpected := e
		short8 := e.result == 0 && (e.operation == "BRANCH_REPORT" || e.operation == "LOCK")
		if short8 && len(e.msg) > 127 {
			nativeExpected.msg = e.msg[:127]
			truncated++
		}
		require(reflect.DeepEqual(manager.Decode(codec.CodecTypeSeata, goBytes), model(nativeExpected)), name+": native Go semantic decode mismatch")
		require(bytes.Equal(manager.Encode(codec.CodecTypeSeata, manager.Decode(codec.CodecTypeSeata, goBytes)), goBytes), name+": native Go roundtrip failed")
		decoded, panicked := safeDecode(manager, java)
		switch {
		case short8:
			lengthDiff++
			require(!bytes.Equal(java, goBytes), name+": expected short8/short16 divergence missing")
			require(panicked || !reflect.DeepEqual(decoded, expected), name+": Go unexpectedly recovered Java failure semantics")
			fmt.Printf("OPEN_LENGTH %s java=%x go=%x panic=%v\n", name, java, goBytes, panicked)
		case e.errorCode == 20:
			ordinalDiff++
			require(!bytes.Equal(java, goBytes), name+": expected fail-fast ordinal divergence missing")
			require(panicked || !reflect.DeepEqual(decoded, expected), name+": ordinal semantics unexpectedly equal")
			fmt.Printf("OPEN_ORDINAL %s Java20_vs_Go19 java=%x go=%x\n", name, java, goBytes)
		default:
			require(bytes.Equal(java, goBytes) && !panicked && reflect.DeepEqual(decoded, expected), name+": unexpected compatibility failure")
			if e.status > 15 {
				opaque++
				fmt.Printf("OPAQUE_STATUS %s code=%d bytes/fields preserved; pinned Go has no corresponding named GlobalStatus\n", name, e.status)
			} else {
				compatible++
				fmt.Printf("COMPATIBLE %s bytes=%d\n", name, len(java))
			}
		}
	}
	require(compatible == 62 && lengthDiff == 6 && ordinalDiff == 1 && opaque == 9, "missing/unexpected diagnostic outcomes")
	fmt.Printf("DIAGNOSTIC PASS:78 native Go roundtrips; compatible=%d OPEN_length=%d OPEN_ordinal=%d opaque_status=%d truncated_error_text=%d. No blanket interoperability claim.\n", compatible, lengthDiff, ordinalDiff, opaque, truncated)
}

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
