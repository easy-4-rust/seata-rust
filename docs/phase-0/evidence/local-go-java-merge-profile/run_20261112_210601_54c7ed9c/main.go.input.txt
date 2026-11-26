// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"strconv"
	"strings"
	"time"
)

type mergeRow struct {
	Name string `json:"name"`
	Hex  string `json:"serialized_hex"`
	Type int    `json:"type_id"`
}

func readMergeRows(path string) []mergeRow {
	raw, err := os.ReadFile(path)
	if err != nil {
		panic(err)
	}
	var rows []mergeRow
	if err = json.Unmarshal(raw, &rows); err != nil {
		panic(err)
	}
	return rows
}
func controlRequests(ids bool) message.MergedWarpMessage {
	value := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{message.GlobalBeginRequest{Timeout: 2000 * time.Millisecond, TransactionName: "m-a"}, message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: "a", ExtraData: []byte{}}}}}
	if ids {
		value.MsgIds = []int32{7, -9}
	}
	return value
}
func wantedMerge(name string, ordinary []message.MessageTypeAware) message.MessageTypeAware {
	if strings.HasPrefix(name, "SignedTimeout_") {
		millis, err := strconv.ParseInt(strings.TrimPrefix(name, "SignedTimeout_"), 10, 64)
		if err != nil {
			panic(err)
		}
		return message.MergedWarpMessage{Msgs: []message.MessageTypeAware{message.GlobalBeginRequest{Timeout: time.Duration(millis) * time.Millisecond, TransactionName: "signed"}}}
	}
	if name == "AbsentTMVersion" {
		return message.MergedWarpMessage{Msgs: []message.MessageTypeAware{message.RegisterTMResponse{}}}
	}
	if name == "AbsentRMVersion" {
		return message.MergedWarpMessage{Msgs: []message.MessageTypeAware{message.RegisterRMResponse{}}}
	}
	if name == "NullReportStatus" || name == "UnknownReportStatus" {
		status := message.GlobalStatus(0)
		if name == "NullReportStatus" {
			status = 255
		}
		return message.MergedWarpMessage{Msgs: []message.MessageTypeAware{message.GlobalReportRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: "ctx", ExtraData: []byte("meta")}, GlobalStatus: status}}}
	}

	all := message.MergedWarpMessage{Msgs: ordinary}
	all.MsgIds = make([]int32, len(ordinary))
	var resultItems []message.MessageTypeAware
	for i, item := range ordinary {
		all.MsgIds[i] = int32(i + 1)
		if i%2 == 1 {
			all.MsgIds[i] = -all.MsgIds[i]
		}
		kind := int(item.GetTypeCode())
		if kind == 2 || kind == 4 || kind == 6 || kind == 8 || kind == 10 || kind == 12 || kind == 14 || kind == 16 || kind == 18 || kind == 22 || kind == 102 || kind == 104 {
			resultItems = append(resultItems, item)
		}
	}
	results := message.MergeResultMessage{Msgs: resultItems}
	if strings.HasPrefix(name, "Single_") {
		for _, item := range ordinary {
			if name == fmt.Sprintf("Single_%d", item.GetTypeCode()) {
				return message.MergedWarpMessage{Msgs: []message.MessageTypeAware{item}, MsgIds: []int32{-100 - int32(item.GetTypeCode())}}
			}
		}
		panic("unknown Single type")
	}
	switch name {
	case "All25WithIds":
		return all
	case "All25WithoutIds":
		return message.MergedWarpMessage{Msgs: ordinary}
	case "All12Results":
		return results
	case "All250ExceptionResults":
		var items []message.MessageTypeAware
		for _, kind := range []int{2, 4, 6, 8, 10, 12, 14, 16, 18, 22} {
			for code := 0; code < 25; code++ {
				items = append(items, sample(kind, code))
			}
		}
		return message.MergeResultMessage{Msgs: items}
	case "NestedAll":
		return message.MergedWarpMessage{Msgs: []message.MessageTypeAware{all, results}, MsgIds: []int32{77, -77}}
	case "MergeRequestWithIds":
		return controlRequests(true)
	case "MergeRequestWithoutIds":
		return controlRequests(false)
	case "MergeRequestEmpty":
		return message.MergedWarpMessage{Msgs: []message.MessageTypeAware{}}
	case "MergeRequestNested":
		return message.MergedWarpMessage{Msgs: []message.MessageTypeAware{controlRequests(false)}, MsgIds: []int32{42}}
	case "MergeResultEmpty":
		return message.MergeResultMessage{Msgs: []message.MessageTypeAware{}}
	case "MergeResult":
		return message.MergeResultMessage{Msgs: []message.MessageTypeAware{message.GlobalBeginResponse{AbstractTransactionResponse: message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeSuccess}}, Xid: "a", ExtraData: []byte{}}, message.GlobalCommitResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeSuccess}}, GlobalStatus: message.GlobalStatusCommitted}}}}
	default:
		panic("unknown merge fixture")
	}
}
func main() {
	codec.Init()
	native := codec.GetCodecManager()
	p, err := codec.NewJavaV1MessageProfile(native)
	if err != nil {
		panic(err)
	}
	var ordinary []message.MessageTypeAware
	seenTypes := map[int]bool{}
	plain := readMergeRows(os.Args[1])
	if len(plain) != 29 {
		panic("need29 ordinary fixtures")
	}
	for _, row := range plain {
		wire, err := hex.DecodeString(row.Hex)
		if err != nil {
			panic(err)
		}
		expected := native.Decode(codec.CodecTypeSeata, wire).(message.MessageTypeAware)
		encoded, err := p.Encode(expected)
		if err != nil || !bytes.Equal(encoded, wire) {
			panic("ordinary encode mismatch: " + row.Name)
		}
		decoded, err := p.Decode(wire)
		if err != nil || !reflect.DeepEqual(decoded, expected) {
			panic("ordinary fields mismatch: " + row.Name)
		}
		if !seenTypes[row.Type] {
			ordinary = append(ordinary, expected)
			seenTypes[row.Type] = true
		}
	}
	if len(ordinary) != 25 {
		panic("all25 native models required")
	}
	fresh := readMergeRows(os.Args[2])
	if len(fresh) != 38 {
		panic("need38 fresh Java cases")
	}
	original := readMergeRows(os.Args[3])
	var rows []mergeRow
	rows = append(rows, fresh...)
	for _, row := range original {
		if row.Hex != "" {
			rows = append(rows, row)
		}
	}
	if len(rows) != 44 {
		panic("need all44 merge cases")
	}
	seen := map[string]bool{}
	for _, row := range rows {
		if seen[row.Name] {
			panic("duplicate merge")
		}
		seen[row.Name] = true
		want := wantedMerge(row.Name, ordinary)
		wire, err := hex.DecodeString(row.Hex)
		if err != nil {
			panic(err)
		}
		before, _ := json.Marshal(want)
		decoded, err := p.Decode(wire)
		if err != nil {
			panic(err)
		}
		fields := reflect.DeepEqual(decoded, want)
		semantics := true
		if row.Name == "AbsentTMVersion" || row.Name == "AbsentRMVersion" {
			info, err := p.RegistrationVersion(decoded.(message.MergedWarpMessage).Msgs[0], "2.8.0-SNAPSHOT")
			semantics = err == nil && info.WireVersion == nil && info.JavaVersion == "2.8.0-SNAPSHOT"
		}
		if row.Name == "NullReportStatus" || row.Name == "UnknownReportStatus" {
			status, present, err := p.ReportStatus(decoded.(message.MergedWarpMessage).Msgs[0])
			semantics = err == nil && status == 0 && present == (row.Name == "UnknownReportStatus")
		}

		encoded, encerr := p.Encode(want)
		ptr := reflect.New(reflect.TypeOf(want))
		ptr.Elem().Set(reflect.ValueOf(want))
		pointer, ptrerr := p.Encode(ptr.Interface())
		after, _ := json.Marshal(want)
		back, backerr := p.Encode(decoded)
		record := map[string]interface{}{"name": row.Name, "fields_match": fields, "java_semantics_match": semantics, "bytes_match": encerr == nil && bytes.Equal(encoded, wire), "pointer_match": ptrerr == nil && bytes.Equal(pointer, wire), "caller_unchanged": bytes.Equal(before, after), "roundtrip_match": backerr == nil && bytes.Equal(back, wire)}
		for key, value := range record {
			if key != "name" && value != true {
				panic(fmt.Sprintf("merge %s failed%s", row.Name, key))
			}
		}
		raw, _ := json.Marshal(record)
		fmt.Println("SEATA_MERGE_RESULT " + string(raw))
	}
	if native.GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMerge) != nil || native.GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMergeResult) != nil {
		panic("native registry changed")
	}
	fmt.Println("MERGE PROFILE PASS:29 ordinary payloads,all25 models,38 fresh+6 original merges,all250 nested exception semantics; native registry unchanged")
}
