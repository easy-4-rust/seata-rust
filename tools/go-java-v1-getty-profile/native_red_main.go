// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
)

func main() {
	raw, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	var rows []struct {
		Name    string `json:"name"`
		Frame   string `json:"frame_hex"`
		Payload string `json:"serialized_hex"`
	}
	if err = json.Unmarshal(raw, &rows); err != nil {
		panic(err)
	}
	if len(rows) != 9 {
		panic("need nine original Java controls")
	}
	codec.Init()
	h := &getty.RpcPackageHandler{}
	missing := 0
	for _, r := range rows {
		wire, err := hex.DecodeString(r.Frame)
		if err != nil {
			panic(err)
		}
		value, n, err := h.Read(nil, wire)
		if err != nil || n != len(wire) {
			panic("not the expected complete-frame baseline")
		}
		if r.Payload != "" {
			if value.(message.RpcMessage).Body != nil {
				panic("baseline unexpectedly supports merge")
			}
			missing++
			fmt.Println("GETTY RED missing typed body: " + r.Name)
		}
	}
	if missing != 6 {
		panic("need six behavioral failures")
	}
	if _, _, err := h.Read(nil, nil); err == nil {
		panic("baseline empty input unexpectedly waits")
	}
	fmt.Println("GETTY RED empty input returns error rather than waiting")
	os.Exit(1)
}
