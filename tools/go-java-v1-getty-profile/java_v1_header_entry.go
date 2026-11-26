// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package message

// JavaV1HeaderEntry 保留V1头字段的原始顺序与可空值。对应Java: ProtocolEncoderV1/ProtocolDecoderV1的headMap。
// Value=nil表示有符号长度-1，指向空串表示长度0；空Key有效，null Key不支持。
type JavaV1HeaderEntry struct {
	Key   string
	Value *string
}
