// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import "seata.apache.org/seata-go/v2/pkg/protocol/codec"

func main() {
	codec.Init()
	manager := codec.GetCodecManager()
	run(func(v interface{}) ([]byte, error) { return manager.Encode(codec.CodecTypeSeata, v), nil },
		func(wire []byte) (interface{}, error) { return manager.Decode(codec.CodecTypeSeata, wire), nil })
}
