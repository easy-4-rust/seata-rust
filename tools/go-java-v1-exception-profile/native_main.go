// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import "seata.apache.org/seata-go/v2/pkg/protocol/codec"

func main() {
	codec.Init()
	m := codec.GetCodecManager()
	verify(func(in interface{}) ([]byte, error) { return m.Encode(codec.CodecTypeSeata, in), nil }, func(in []byte) (interface{}, error) { return m.Decode(codec.CodecTypeSeata, in), nil }, false)
}
