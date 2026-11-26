// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
// 验证程序只调用显式适配公开API；不注册或修改默认codec。
package main

import (
	"bufio"
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
	"strings"
	"time"
)

func text(value string) string { return hex.EncodeToString([]byte(value)) }
func optional(value string) interface{} {
	if value == "" {
		return nil
	}
	return text(value)
}
func raw(value []byte) interface{} { return optional(string(value)) }
func response(v message.AbstractTransactionResponse) []interface{} {
	// 独立预期域表：原Go域值不等同Java码，不能将Go枚举直接截断成byte。
	domains := []int32{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 256, 19, 257, 258, 259, 260}
	code := -1
	for index, domain := range domains {
		if int32(v.TransactionErrorCode) == domain {
			code = index
			break
		}
	}
	if code < 0 {
		panic("unmodeled exception domain")
	}
	return []interface{}{int(v.ResultCode), optional(v.Msg), code}
}
func more(base []interface{}, tail ...interface{}) []interface{} { return append(base, tail...) }
func branch(v message.AbstractBranchEndRequest) []interface{} {
	return []interface{}{text(v.Xid), v.BranchId, int(v.BranchType), text(v.ResourceId), raw(v.ApplicationData)}
}
func branchResponse(v message.AbstractBranchEndResponse) []interface{} {
	return more(response(v.AbstractTransactionResponse), optional(v.Xid), v.BranchId, int(v.BranchStatus))
}
func register(v message.BranchRegisterRequest) []interface{} {
	return []interface{}{optional(v.Xid), int(v.BranchType), optional(v.ResourceId), optional(v.LockKey), raw(v.ApplicationData)}
}
func end(v message.AbstractGlobalEndRequest) []interface{} {
	return []interface{}{optional(v.Xid), raw(v.ExtraData)}
}
func endResponse(v message.AbstractGlobalEndResponse) []interface{} {
	return more(response(v.AbstractTransactionResponse), int(v.GlobalStatus))
}
func tree(p *codec.JavaV1MessageProfile, value interface{}) interface{} {
	var fields []interface{}
	switch v := value.(type) {
	case message.GlobalBeginRequest:
		fields = []interface{}{int64(v.Timeout / time.Millisecond), optional(v.TransactionName)}
	case message.GlobalBeginResponse:
		fields = more(response(v.AbstractTransactionResponse), optional(v.Xid), raw(v.ExtraData))
	case message.GlobalCommitRequest:
		fields = end(v.AbstractGlobalEndRequest)
	case message.GlobalRollbackRequest:
		fields = end(v.AbstractGlobalEndRequest)
	case message.GlobalStatusRequest:
		fields = end(v.AbstractGlobalEndRequest)
	case message.GlobalReportRequest:
		status, present, err := p.ReportStatus(v)
		if err != nil {
			panic(err)
		}
		var wire interface{}
		if present {
			wire = int(status)
		}
		fields = more(end(v.AbstractGlobalEndRequest), wire)
	case message.GlobalCommitResponse:
		fields = endResponse(v.AbstractGlobalEndResponse)
	case message.GlobalRollbackResponse:
		fields = endResponse(v.AbstractGlobalEndResponse)
	case message.GlobalStatusResponse:
		fields = endResponse(v.AbstractGlobalEndResponse)
	case message.GlobalReportResponse:
		fields = endResponse(v.AbstractGlobalEndResponse)
	case message.BranchCommitRequest:
		fields = branch(v.AbstractBranchEndRequest)
	case message.BranchRollbackRequest:
		fields = branch(v.AbstractBranchEndRequest)
	case message.BranchCommitResponse:
		fields = branchResponse(v.AbstractBranchEndResponse)
	case message.BranchRollbackResponse:
		fields = branchResponse(v.AbstractBranchEndResponse)
	case message.BranchRegisterRequest:
		fields = register(v)
	case message.GlobalLockQueryRequest:
		fields = register(v.BranchRegisterRequest)
	case message.BranchRegisterResponse:
		fields = more(response(v.AbstractTransactionResponse), v.BranchId)
	case message.BranchReportRequest:
		fields = []interface{}{optional(v.Xid), v.BranchId, int(v.Status), optional(v.ResourceId), raw(v.ApplicationData), int(v.BranchType)}
	case message.BranchReportResponse:
		fields = response(v.AbstractTransactionResponse)
	case message.GlobalLockQueryResponse:
		fields = more(response(v.AbstractTransactionResponse), v.Lockable)
	case message.RegisterTMRequest:
		fields = []interface{}{text(v.Version), text(v.ApplicationId), text(v.TransactionServiceGroup), text(string(v.ExtraData))}
	case message.RegisterRMRequest:
		fields = []interface{}{text(v.Version), optional(v.ApplicationId), text(v.TransactionServiceGroup), raw(v.ExtraData), optional(v.ResourceIds)}
	case message.RegisterTMResponse:
		info, err := p.RegistrationVersion(v, "2.8.0-SNAPSHOT")
		if err != nil {
			panic(err)
		}
		var wire interface{}
		if info.WireVersion != nil {
			wire = text(*info.WireVersion)
		}
		fields = []interface{}{v.Identified, wire, text(info.JavaVersion)}
	case message.RegisterRMResponse:
		info, err := p.RegistrationVersion(v, "2.8.0-SNAPSHOT")
		if err != nil {
			panic(err)
		}
		var wire interface{}
		if info.WireVersion != nil {
			wire = text(*info.WireVersion)
		}
		fields = []interface{}{v.Identified, wire, text(info.JavaVersion)}
	case message.UndoLogDeleteRequest:
		fields = []interface{}{int(v.BranchType), text(v.ResourceId), v.SaveDays}
	case message.MergedWarpMessage:
		children := []interface{}{}
		for _, child := range v.Msgs {
			children = append(children, tree(p, child))
		}
		ids := []int32{}
		ids = append(ids, v.MsgIds...)
		fields = []interface{}{children, ids}
	case message.MergeResultMessage:
		children := []interface{}{}
		for _, child := range v.Msgs {
			children = append(children, tree(p, child))
		}
		fields = []interface{}{children}
	default:
		panic(fmt.Sprintf("unmodeled message %T", value))
	}
	return map[string]interface{}{"type": int(value.(message.MessageTypeAware).GetTypeCode()), "fields": fields}
}
func rpcTree(p *codec.JavaV1MessageProfile, rpc message.RpcMessage) interface{} {
	headers := []interface{}{}
	for _, entry := range rpc.HeaderEntries {
		var value interface{}
		if entry.Value != nil {
			value = text(*entry.Value)
		}
		headers = append(headers, []interface{}{text(entry.Key), value})
	}
	var body interface{}
	if rpc.Type >= 3 {
		heartbeat, ok := rpc.Body.(message.HeartBeatMessage)
		if !ok || heartbeat.Ping != (rpc.Type == 3) {
			panic("heartbeat polarity differs")
		}
	} else {
		body = tree(p, rpc.Body)
	}
	return map[string]interface{}{"id": rpc.ID, "kind": int(rpc.Type), "codec": rpc.Codec, "compressor": rpc.Compressor, "headers": headers, "body": body}
}
func process(id, kind string, wire []byte, p *codec.JavaV1MessageProfile, h *getty.RpcPackageHandler) {
	// panic是独立失败，绝不把崩溃解释为正常拒绝。
	defer func() {
		if problem := recover(); problem != nil {
			fmt.Printf("PEER_RESULT\t%s\tpanic\tnull\t\n", id)
		}
	}()
	before := append([]byte(nil), wire...)
	var value interface{}
	var fields interface{}
	var err error
	var out []byte
	switch kind {
	case "payload":
		value, err = p.Decode(wire)
	case "frame":
		var consumed int
		value, consumed, err = h.Read(nil, wire)
		// 本包装器的比较单位是一条完整帧；Getty本身的等待/消费一帧语义保留。
		if err == nil && (value == nil || consumed != len(wire)) {
			err = fmt.Errorf("not exactly one complete frame")
		}
	default:
		panic("unknown input kind")
	}
	if err != nil {
		fmt.Printf("PEER_RESULT\t%s\trejected\tnull\t\n", id)
		return
	}
	modelBefore, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	if kind == "payload" {
		fields = tree(p, value)
		out, err = p.Encode(value)
	} else {
		fields = rpcTree(p, value.(message.RpcMessage))
		out, err = h.Write(nil, value)
	}
	modelAfter, afterErr := json.Marshal(value)
	if err != nil || afterErr != nil || !bytes.Equal(before, wire) || !bytes.Equal(modelBefore, modelAfter) {
		panic("caller mutated or reencoding failed")
	}
	data, err := json.Marshal(fields)
	if err != nil {
		panic(err)
	}
	fmt.Printf("PEER_RESULT\t%s\taccepted\t%s\t%s\n", id, data, hex.EncodeToString(out))
}
func main() {
	if len(os.Args) != 2 {
		panic("one input TSV path required")
	}
	codec.Init()
	manager := codec.GetCodecManager()
	p, err := codec.NewJavaV1MessageProfile(manager)
	if err != nil {
		panic(err)
	}
	handler := &getty.RpcPackageHandler{PayloadCodec: p}
	file, err := os.Open(os.Args[1])
	if err != nil {
		panic(err)
	}
	defer file.Close()
	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 4096), 20*1024*1024)
	for scanner.Scan() {
		cols := strings.Split(scanner.Text(), "\t")
		if len(cols) != 3 {
			panic("three TSV columns required")
		}
		wire, err := hex.DecodeString(cols[2])
		if err != nil {
			panic(err)
		}
		process(cols[0], cols[1], wire, p, handler)
	}
	if err = scanner.Err(); err != nil {
		panic(err)
	}
	if manager.GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMerge) != nil || manager.GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMergeResult) != nil {
		panic("native registry changed")
	}
	fmt.Println("NATIVE_REGISTRY_UNCHANGED")
}
