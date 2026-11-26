// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package codec

import (
	"encoding/binary"
	"fmt"
	"unicode/utf8"
)

// javaV1MessageCursor 是有界字段游标；对应Java各V1消息codec的字段顺序。
type javaV1MessageCursor struct {
	data []byte
	at   int
}

func (c *javaV1MessageCursor) take(n int) ([]byte, error) {
	if n < 0 || n > len(c.data)-c.at {
		return nil, fmt.Errorf("truncated/negative V1 field")
	}
	v := c.data[c.at : c.at+n]
	c.at += n
	return v, nil
}
func (c *javaV1MessageCursor) u8() (byte, error) {
	v, e := c.take(1)
	if e != nil {
		return 0, e
	}
	return v[0], nil
}
func (c *javaV1MessageCursor) u16() (uint16, error) {
	v, e := c.take(2)
	if e != nil {
		return 0, e
	}
	return binary.BigEndian.Uint16(v), nil
}
func (c *javaV1MessageCursor) text(width int, required bool) error {
	v, e := c.take(width)
	if e != nil {
		return e
	}
	var n int
	if width == 2 {
		n = int(int16(binary.BigEndian.Uint16(v)))
	} else {
		n = int(int32(binary.BigEndian.Uint32(v)))
	}
	if required && n == 0 {
		return fmt.Errorf("required V1 field missing")
	}
	text, e := c.take(n)
	if e != nil {
		return e
	}
	if !utf8.Valid(text) {
		return fmt.Errorf("invalid V1 UTF8")
	}
	return nil
}

// 独立字段标记避免把子消息误当成有通用长度前缀的数据块。
const (
	j16 byte = iota
	j32
	jRequired16
	jInt16
	jInt32
	jInt64
	jBranch
	jBranchStatus
	jGlobal
	jNullableGlobal
	jBool8
	jBool16
	jResponse
)

var javaV1Schemas = map[uint16][]byte{
	1: {jInt32, j16}, 2: {jResponse, j16, j16}, 3: {jRequired16, jInt64, jBranch, jRequired16, j32}, 4: {jResponse, j16, jInt64, jBranchStatus},
	5: {jRequired16, jInt64, jBranch, jRequired16, j32}, 6: {jResponse, j16, jInt64, jBranchStatus}, 7: {j16, j16}, 8: {jResponse, jGlobal}, 9: {j16, j16}, 10: {jResponse, jGlobal},
	11: {j16, jBranch, j16, j32, j32}, 12: {jResponse, jInt64}, 13: {j16, jInt64, jBranchStatus, j16, j32, jBranch}, 14: {jResponse}, 15: {j16, j16}, 16: {jResponse, jGlobal}, 17: {j16, j16, jNullableGlobal}, 18: {jResponse, jGlobal}, 21: {j16, jBranch, j16, j32, j32}, 22: {jResponse, jBool16},
	101: {j16, j16, j16, j16}, 102: {jBool8, j16}, 103: {jRequired16, j16, j16, j16, j32}, 104: {jBool8, j16}, 111: {jBranch, jRequired16, jInt16},
}

func javaV1TransactionKind(k uint16) bool {
	return k == 2 || k == 4 || k == 6 || k == 8 || k == 10 || k == 12 || k == 14 || k == 16 || k == 18 || k == 22
}
func javaV1ResultKind(k uint16) bool { return javaV1TransactionKind(k) || k == 102 || k == 104 }
func (c *javaV1MessageCursor) ordinary(k uint16) error {
	schema, ok := javaV1Schemas[k]
	if !ok {
		return fmt.Errorf("unsupported V1 type %d", k)
	}
	for _, field := range schema {
		switch field {
		case j16, jRequired16:
			if e := c.text(2, field == jRequired16); e != nil {
				return e
			}
		case j32:
			if e := c.text(4, false); e != nil {
				return e
			}
		case jInt16, jInt32, jInt64:
			n := 2
			if field == jInt32 {
				n = 4
			}
			if field == jInt64 {
				n = 8
			}
			if _, e := c.take(n); e != nil {
				return e
			}
		case jBool16:
			v, e := c.u16()
			if e != nil {
				return e
			}
			if v > 1 {
				return fmt.Errorf("invalid V1 boolean")
			}
		case jResponse:
			v, e := c.u8()
			if e != nil {
				return e
			}
			if v > 1 {
				return fmt.Errorf("unknown V1 result")
			}
			if v == 0 {
				if e := c.text(2, false); e != nil {
					return e
				}
			}
			code, e := c.u8()
			if e != nil {
				return e
			}
			if code > 24 {
				return fmt.Errorf("unknown Java V1 error")
			}
		default:
			v, e := c.u8()
			if e != nil {
				return e
			}
			limit := byte(20)
			switch field {
			case jBranch:
				limit = 4
			case jBranchStatus:
				limit = 15
			case jBool8:
				limit = 1
			}
			if field == jNullableGlobal && v == 255 {
				continue
			}
			if v > limit {
				return fmt.Errorf("unknown V1 enum/boolean")
			}
		}
	}
	return nil
}
