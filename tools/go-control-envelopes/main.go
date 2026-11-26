// Copyright 2026 Seata-Rust contributors.
// Licensed under the Apache License, Version 2.0.
package main

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
	"strings"
)

type sample struct {
	Name          string `json:"name"`
	FrameHex      string `json:"frame_hex"`
	SerializedHex string `json:"serialized_hex"`
	Kind          byte   `json:"kind"`
	RequestID     int32  `json:"request_id"`
}

func require(ok bool, msg string) {
	if !ok {
		panic(msg)
	}
}

func main() {
	require(len(os.Args) == 2, "one Java runtime fixture JSON path required")
	raw, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	var samples []sample
	if err = json.Unmarshal(raw, &samples); err != nil {
		panic(err)
	}
	require(len(samples) == 9, "exact Java control fixture inventory required")
	codec.Init()
	manager := codec.GetCodecManager()
	require(manager.GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMerge) == nil, "pinned merge codec inventory changed")
	require(manager.GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMergeResult) == nil, "pinned merged-result codec inventory changed")
	handler := &getty.RpcPackageHandler{}
	compatible, unsupported := 0, 0
	for _, item := range samples {
		frame, err := hex.DecodeString(item.FrameHex)
		if err != nil {
			panic(err)
		}
		decoded, n, err := handler.Read(nil, frame)
		if err != nil {
			panic(err)
		}
		require(n == len(frame), "actual Getty consumption differs: "+item.Name)
		rpc := decoded.(message.RpcMessage)
		require(rpc.ID == item.RequestID && byte(rpc.Type) == item.Kind, "outer correlation differs: "+item.Name)
		result := map[string]interface{}{"name": item.Name, "consumed": n, "request_id": rpc.ID}
		if strings.HasPrefix(item.Name, "Heartbeat") {
			body := rpc.Body.(message.HeartBeatMessage)
			require(body.Ping == (item.Name != "HeartbeatPong"), "heartbeat flag differs")
			if item.Name == "HeartbeatHeadMap" {
				require(rpc.HeadMap["traceId"] == "hb", "actual header map differs")
			}
			encoded, err := handler.Write(nil, rpc)
			if err != nil {
				panic(err)
			}
			require(bytes.Equal(encoded, frame), "actual Getty heartbeat bytes differ")
			result["classification"] = "byte-compatible"
			compatible++
		} else {
			require(rpc.Body == nil, "missing pinned merge codec unexpectedly decoded a body")
			var empty message.MessageTypeAware = message.MergedWarpMessage{}
			if strings.HasPrefix(item.Name, "MergeResult") {
				empty = message.MergeResultMessage{}
			}
			encoded := manager.Encode(codec.CodecTypeSeata, empty)
			require(len(encoded) == 0, "missing pinned merge codec unexpectedly encoded payload")
			result["classification"] = "unsupported-pinned-go-codec"
			result["issue"] = "GO-V1-MERGE-001"
			unsupported++
		}
		encoded, err := json.Marshal(result)
		if err != nil {
			panic(err)
		}
		fmt.Println("SEATA_GO_CONTROL " + string(encoded))
	}
	require(compatible == 3 && unsupported == 6, "fixture classification inventory changed")
	fmt.Printf("PASS control diagnostic heartbeats=%d missingMergeCodecs=%d; merge interoperability remains OPEN\n", compatible, unsupported)
}
