// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package main

import "seata.apache.org/seata-go/v2/pkg/protocol/codec"

func main() {
	codec.Init()
	profile, err := codec.NewJavaV1ResponseProfile(codec.GetCodecManager())
	must(err)
	run(profile.Encode, profile.Decode)
}
