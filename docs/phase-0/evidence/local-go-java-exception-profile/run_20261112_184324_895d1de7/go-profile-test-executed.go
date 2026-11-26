// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package codec_test

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"os"
	"reflect"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	e "seata.apache.org/seata-go/v2/pkg/util/errors"
	"strings"
	"testing"
)

func profile(t *testing.T) *codec.JavaV1ResponseProfile {
	t.Helper()
	codec.Init()
	p, err := codec.NewJavaV1ResponseProfile(codec.GetCodecManager())
	if err != nil {
		t.Fatal(err)
	}
	return p
}
func register(code e.TransactionErrorCode) message.BranchRegisterResponse {
	return message.BranchRegisterResponse{AbstractTransactionResponse: message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeFailed, Msg: "fail fast"}, TransactionErrorCode: code}, BranchId: 0x0102030405060708}
}

func TestJavaV1ExceptionProfileKeepsOriginalDomainAndRegistry(t *testing.T) {
	codec.Init()
	manager := codec.GetCodecManager()
	value := register(e.LockKeyConflictFailFast)
	before := manager.Encode(codec.CodecTypeSeata, value)
	p := profile(t)
	wire, err := p.Encode(&value)
	if err != nil {
		t.Fatal(err)
	}
	want, _ := hex.DecodeString("000c0000096661696c2066617374140102030405060708")
	if !bytes.Equal(wire, want) {
		t.Fatalf("wrong Java fail-fast bytes: %x", wire)
	}
	if !bytes.Equal(manager.Encode(codec.CodecTypeSeata, value), before) {
		t.Fatal("profile mutated default registry or input")
	}
	if before[14] != 19 || value.TransactionErrorCode != e.LockKeyConflictFailFast {
		t.Fatal("legacy fail-fast code changed")
	}
	decoded, err := p.Decode(wire)
	if err != nil || !reflect.DeepEqual(decoded, value) {
		t.Fatal("wrong fail-fast domain meaning")
	}
	other := append([]byte(nil), wire...)
	other[14] = 19
	decoded, err = p.Decode(other)
	if err != nil {
		t.Fatal(err)
	}
	if decoded.(message.BranchRegisterResponse).TransactionErrorCode != e.JavaV1NotRaftLeader {
		t.Fatal("Java19 aliased to Go fail-fast")
	}
}

func TestJavaV1ExceptionProfileRejectsGoOnlyErrors(t *testing.T) {
	p := profile(t)
	for _, code := range []e.TransactionErrorCode{e.TccFenceDbDuplicateKeyError, e.RollbackFenceError, e.CommitFenceError, e.TccFenceDbError, e.PrepareFenceError, e.FenceBusinessError, e.FencePhaseError, e.SQLUndoDirtyError, e.ObjectNotExists, e.StateMachineInstanceNotExists, e.ContextVariableReplayFailed, e.InvalidParameter, e.OperationDenied, e.ForwardInvalid, e.AsynchronousStartDisabled, -1, 255, 261, 1000, 2147483647} {
		if wire, err := p.Encode(register(code)); err == nil || wire != nil {
			t.Fatalf("unsupported Go error %d accepted", code)
		}
	}
}

func TestJavaV1ExceptionProfileRejectsMalformed250CaseMatrix(t *testing.T) {
	p := profile(t)
	raw, err := os.ReadFile(os.Getenv("SEATA_EXCEPTION_VECTORS"))
	if err != nil {
		t.Fatal(err)
	}
	var rows []struct {
		Name string `json:"name"`
		Hex  string `json:"serialized_hex"`
	}
	if err = json.Unmarshal(raw, &rows); err != nil {
		t.Fatal(err)
	}
	if len(rows) != 250 {
		t.Fatal("need250 cases")
	}
	for _, row := range rows {
		wire, err := hex.DecodeString(row.Hex)
		if err != nil {
			t.Fatal(err)
		}
		for at := 0; at < len(wire); at++ {
			if v, err := p.Decode(wire[:at]); err == nil || v != nil {
				t.Fatalf("%s truncation%d accepted", row.Name, at)
			}
		}
		if v, err := p.Decode(append(append([]byte(nil), wire...), 0)); err == nil || v != nil {
			t.Fatal("trailing bytes accepted")
		}
		errorAt := 5 + int(binary.BigEndian.Uint16(wire[3:5]))
		for code := 25; code <= 255; code++ {
			bad := append([]byte(nil), wire...)
			bad[errorAt] = byte(code)
			if v, err := p.Decode(bad); err == nil || v != nil {
				t.Fatalf("unknown wire error%d accepted", code)
			}
		}
		for _, change := range []func([]byte){func(b []byte) { b[2] = 2 }, func(b []byte) { b[3] = 0x80; b[4] = 0 }, func(b []byte) { b[5] = 0xff }} {
			bad := append([]byte(nil), wire...)
			change(bad)
			if v, err := p.Decode(bad); err == nil || v != nil {
				t.Fatalf("%s malformed field accepted", row.Name)
			}
		}
		if strings.HasPrefix(row.Name, "22_") {
			bad := append([]byte(nil), wire...)
			bad[len(bad)-1] = 2
			if _, err := p.Decode(bad); err == nil {
				t.Fatal("bad lockable accepted")
			}
		}
	}
	if v, err := p.Decode(make([]byte, 1024*1024+1)); err == nil || v != nil {
		t.Fatal("oversized payload accepted")
	}
}

func TestJavaV1ExceptionProfileValidatesInputsAndDoesNotTruncate(t *testing.T) {
	p := profile(t)
	if _, err := codec.NewJavaV1ResponseProfile(nil); err == nil {
		t.Fatal("nil manager accepted")
	}
	var missing *message.BranchRegisterResponse
	for _, value := range []interface{}{nil, missing, message.GlobalBeginRequest{}} {
		if v, err := p.Encode(value); err == nil || v != nil {
			t.Fatal("non-response/nil accepted")
		}
	}
	value := register(e.TransactionErrorCodeUnknown)
	for _, text := range []string{string([]byte{0xff}), strings.Repeat("x", 32768), strings.Repeat("错", 10923)} {
		value.Msg = text
		if v, err := p.Encode(value); err == nil || v != nil {
			t.Fatal("invalid/oversized text silently accepted")
		}
	}
	value.Msg = strings.Repeat("错", 10922)
	if _, err := p.Encode(value); err != nil {
		t.Fatal(err)
	}
	value.Msg = "x"
	value.ResultCode = message.ResultCode(2)
	if _, err := p.Encode(value); err == nil {
		t.Fatal("invalid result accepted")
	}
	value.ResultCode = message.ResultCodeSuccess
	if _, err := p.Encode(value); err == nil {
		t.Fatal("success text silently dropped")
	}
	global := message.GlobalCommitResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: register(0).AbstractTransactionResponse, GlobalStatus: message.GlobalStatus(255)}}
	if _, err := p.Encode(global); err == nil {
		t.Fatal("lossy global status cast accepted")
	}
	zero := &codec.JavaV1ResponseProfile{}
	if _, err := zero.Encode(register(0)); err == nil {
		t.Fatal("zero profile accepted")
	}
	if _, err := zero.Decode([]byte{0, 14, 1, 0}); err == nil {
		t.Fatal("zero decode profile accepted")
	}
}

func TestJavaV1ExceptionProfileSuccessAndOptionalText(t *testing.T) {
	p := profile(t)
	base := message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeSuccess}, TransactionErrorCode: e.TransactionErrorCodeUnknown}
	for _, test := range []struct {
		value interface{}
		hex   string
	}{
		{message.BranchReportResponse{AbstractTransactionResponse: base}, "000e0100"},
		{message.GlobalLockQueryResponse{AbstractTransactionResponse: base, Lockable: true}, "001601000001"},
		{message.GlobalLockQueryResponse{AbstractTransactionResponse: base, Lockable: false}, "001601000000"},
	} {
		want, _ := hex.DecodeString(test.hex)
		got, err := p.Encode(test.value)
		if err != nil || !bytes.Equal(got, want) {
			t.Fatal("success field/byte mismatch")
		}
		back, err := p.Decode(want)
		if err != nil || !reflect.DeepEqual(back, test.value) {
			t.Fatal("success semantics lost")
		}
	}
}
