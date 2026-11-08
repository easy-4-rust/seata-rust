// Copyright 2026 Seata-Rust contributors.
// Licensed under the Apache License, Version 2.0.
package main

import (
	"encoding/hex"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"strings"
	"time"

	"seata.apache.org/seata-go/v2/pkg/protocol/branch"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	seataerrors "seata.apache.org/seata-go/v2/pkg/util/errors"
)

type expectedFrame struct {
	Name string `json:"name"`
	SourceVector string `json:"source_vector"`
	RequestType byte `json:"request_type"`
	RequestID uint32 `json:"request_id"`
	SerializerID byte `json:"serializer_id"`
	CompressorID byte `json:"compressor_id"`
	FrameHex string `json:"frame_hex"`
}

type expectedVector struct {
	Name    string `json:"name"`
	TypeID  int    `json:"type_id"`
	BodyHex string `json:"body_hex"`
	SerializedHex string `json:"serialized_hex"`
}

func txResponse() message.AbstractTransactionResponse {
	return message.AbstractTransactionResponse{
		AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeSuccess},
		TransactionErrorCode: seataerrors.TransactionErrorCodeUnknown,
	}
}

func main() {
	const xid = "127.0.0.1:8091:1"
	const resourceID = "mysql://orders"
	const lockKey = "orders:42"
	const branchID int64 = 0x0102030405060708

	identity := message.AbstractIdentifyRequest{
		Version: "V1,0", ApplicationId: "orders-rust",
		TransactionServiceGroup: "default_tx_group", ExtraData: []byte{},
	}
	identifyResponse := message.AbstractIdentifyResponse{Identified: true, Version: "V1,0"}
	branchEnd := message.AbstractBranchEndRequest{
		Xid: xid, BranchId: branchID, BranchType: branch.BranchTypeAT,
		ResourceId: resourceID, ApplicationData: []byte{},
	}
	globalEnd := message.AbstractGlobalEndRequest{Xid: xid, ExtraData: []byte{}}
	registeredBranch := message.BranchRegisterRequest{
		Xid: xid, BranchType: branch.BranchTypeAT, ResourceId: resourceID,
		LockKey: lockKey, ApplicationData: []byte{},
	}

	vectors := []struct {
		name string
		msg  message.MessageTypeAware
	}{
		{"GlobalBeginRequest", message.GlobalBeginRequest{Timeout: 2000 * time.Millisecond, TransactionName: "SeataGoTransaction"}},
		{"GlobalBeginResponse", message.GlobalBeginResponse{AbstractTransactionResponse: txResponse(), Xid: xid, ExtraData: []byte{}}},
		{"BranchCommitRequest", message.BranchCommitRequest{AbstractBranchEndRequest: branchEnd}},
		{"BranchCommitResponse", message.BranchCommitResponse{AbstractBranchEndResponse: message.AbstractBranchEndResponse{AbstractTransactionResponse: txResponse(), Xid: xid, BranchId: branchID, BranchStatus: branch.BranchStatusPhasetwoCommitted}}},
		{"BranchRollbackRequest", message.BranchRollbackRequest{AbstractBranchEndRequest: branchEnd}},
		{"BranchRollbackResponse", message.BranchRollbackResponse{AbstractBranchEndResponse: message.AbstractBranchEndResponse{AbstractTransactionResponse: txResponse(), Xid: xid, BranchId: branchID, BranchStatus: branch.BranchStatusPhasetwoRollbacked}}},
		{"GlobalCommitRequest", message.GlobalCommitRequest{AbstractGlobalEndRequest: globalEnd}},
		{"GlobalCommitResponse", message.GlobalCommitResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: txResponse(), GlobalStatus: message.GlobalStatusCommitted}}},
		{"GlobalRollbackRequest", message.GlobalRollbackRequest{AbstractGlobalEndRequest: globalEnd}},
		{"GlobalRollbackResponse", message.GlobalRollbackResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: txResponse(), GlobalStatus: message.GlobalStatusRollbacked}}},
		{"BranchRegisterRequest", registeredBranch},
		{"BranchRegisterResponse", message.BranchRegisterResponse{AbstractTransactionResponse: txResponse(), BranchId: branchID}},
		{"BranchReportRequest", message.BranchReportRequest{Xid: xid, BranchId: branchID, Status: branch.BranchStatusPhaseoneDone, ResourceId: resourceID, ApplicationData: []byte("trace=abc"), BranchType: branch.BranchTypeAT}},
		{"BranchReportResponse", message.BranchReportResponse{AbstractTransactionResponse: txResponse()}},
		{"GlobalStatusRequest", message.GlobalStatusRequest{AbstractGlobalEndRequest: globalEnd}},
		{"GlobalStatusResponse", message.GlobalStatusResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: txResponse(), GlobalStatus: message.GlobalStatusCommitted}}},
		{"GlobalReportRequest", message.GlobalReportRequest{AbstractGlobalEndRequest: globalEnd, GlobalStatus: message.GlobalStatusBegin}},
		{"GlobalReportResponse", message.GlobalReportResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: txResponse(), GlobalStatus: message.GlobalStatusBegin}}},
		{"GlobalLockQueryRequest", message.GlobalLockQueryRequest{BranchRegisterRequest: registeredBranch}},
		{"GlobalLockQueryResponse", message.GlobalLockQueryResponse{AbstractTransactionResponse: txResponse(), Lockable: true}},
		{"RegisterTMRequest", message.RegisterTMRequest{AbstractIdentifyRequest: identity}},
		{"RegisterTMResponse", message.RegisterTMResponse{AbstractIdentifyResponse: identifyResponse}},
		{"RegisterRMRequest", message.RegisterRMRequest{AbstractIdentifyRequest: identity, ResourceIds: resourceID}},
		{"RegisterRMResponse", message.RegisterRMResponse{AbstractIdentifyResponse: identifyResponse}},
		{"UndoLogDeleteRequest", message.UndoLogDeleteRequest{BranchType: branch.BranchTypeAT, ResourceId: resourceID, SaveDays: 7}},
	}

	expectedBytes, err := os.ReadFile("expected.json")
	must(err)
	var expected []expectedVector
	must(json.Unmarshal(expectedBytes, &expected))
	if len(expected) != len(vectors) {
		fail("fixture count mismatch: expected %d, constructed %d", len(expected), len(vectors))
	}

	codec.Init()
	manager := codec.GetCodecManager()
	serializedByName := make(map[string][]byte, len(vectors))
	for i, vector := range vectors {
		want := expected[i]
		if vector.name != want.Name {
			fail("fixture order mismatch at %d: got %s want %s", i, vector.name, want.Name)
		}
		typeID := int(vector.msg.GetTypeCode())
		if typeID != want.TypeID {
			fail("%s: message type id got %d want %d", vector.name, typeID, want.TypeID)
		}
		messageCodec := manager.GetCodec(codec.CodecTypeSeata, vector.msg.GetTypeCode())
		if messageCodec == nil {
			fail("%s: codec not registered", vector.name)
		}
		body := messageCodec.Encode(vector.msg)
		gotHex := hex.EncodeToString(body)
		wantHex := strings.ToLower(strings.ReplaceAll(want.BodyHex, " ", ""))
		if gotHex != wantHex {
			fail("%s: codec payload mismatch\n got: %s\nwant: %s", vector.name, gotHex, wantHex)
		}

		serialized := manager.Encode(codec.CodecTypeSeata, vector.msg)
		gotSerializedHex := hex.EncodeToString(serialized)
		wantSerializedHex := strings.ToLower(strings.ReplaceAll(want.SerializedHex, " ", ""))
		if gotSerializedHex != wantSerializedHex {
			fail("%s: Seata serializer payload mismatch\n got: %s\nwant: %s", vector.name, gotSerializedHex, wantSerializedHex)
		}
		serializedByName[vector.name] = serialized
		decoded := messageCodec.Decode(body)
		if !reflect.DeepEqual(vector.msg, decoded) {
			fail("%s: decode result differs from source fixture: %#v", vector.name, decoded)
		}
		roundTrip := messageCodec.Encode(decoded)
		if !reflect.DeepEqual(body, roundTrip) {
			fail("%s: decode/re-encode is not byte stable", vector.name)
		}
		fmt.Printf("PASS %-28s type=%3d bytes=%d body=%s\n", vector.name, typeID, len(body), gotHex)
	}
	frameBytes, err := os.ReadFile("frames.json")
	must(err)
	var frames []expectedFrame
	must(json.Unmarshal(frameBytes, &frames))
	for _, expectedFrame := range frames {
		payload, ok := serializedByName[expectedFrame.SourceVector]
		if !ok {
			fail("%s: unknown source vector %s", expectedFrame.Name, expectedFrame.SourceVector)
		}
		frame := make([]byte, 16+len(payload))
		frame[0], frame[1], frame[2] = 0xda, 0xda, 1
		binary.BigEndian.PutUint32(frame[3:7], uint32(len(frame)))
		binary.BigEndian.PutUint16(frame[7:9], 16)
		frame[9] = expectedFrame.RequestType
		frame[10] = expectedFrame.SerializerID
		frame[11] = expectedFrame.CompressorID
		binary.BigEndian.PutUint32(frame[12:16], expectedFrame.RequestID)
		copy(frame[16:], payload)
		got := hex.EncodeToString(frame)
		want := strings.ToLower(strings.ReplaceAll(expectedFrame.FrameHex, " ", ""))
		if got != want {
			fail("%s: V1 frame mismatch\n got: %s\nwant: %s", expectedFrame.Name, got, want)
		}
		fmt.Printf("PASS %-28s frame-bytes=%d\n", expectedFrame.Name, len(frame))
	}
	fmt.Printf("\nGo codec + V1 frame fixture verification passed: %d payloads, %d frames\n", len(vectors), len(frames))
}

func must(err error) {
	if err != nil {
		fail("%v", err)
	}
}

func fail(format string, args ...interface{}) {
	fmt.Fprintf(os.Stderr, "FAIL: "+format+"\n", args...)
	os.Exit(1)
}
