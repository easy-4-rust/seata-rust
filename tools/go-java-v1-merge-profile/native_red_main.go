// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
)

func main() {
	raw, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	var rows []struct {
		Name string `json:"name"`
		Hex  string `json:"serialized_hex"`
	}
	if err = json.Unmarshal(raw, &rows); err != nil {
		panic(err)
	}
	codec.Init()
	missing := 0
	for _, r := range rows {
		if r.Hex == "" {
			continue
		}
		wire, err := hex.DecodeString(r.Hex)
		if err != nil {
			panic(err)
		}
		value := codec.GetCodecManager().Decode(codec.CodecTypeSeata, wire)
		if value == nil {
			missing++
			fmt.Println("MERGE RED missing typed body: " + r.Name)
		} else {
			panic("baseline unexpectedly has merge support")
		}
	}
	if missing != 6 {
		panic("need exact six original Java merge cases")
	}
	fmt.Println("MERGE RED: six actual Java payloads have no original Go codec")
	os.Exit(1)
}
