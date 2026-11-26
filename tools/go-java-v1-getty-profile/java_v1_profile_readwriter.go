// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import (
	"encoding/binary"
	"fmt"
	"sort"
	"unicode/utf8"

	"seata.apache.org/seata-go/v2/pkg/protocol/message"
)

// JavaV1PayloadCodec 是实际Getty帧入口显式注入的严格载荷编解码契约。
// 对应Java: Serializer#serialize/deserialize；错误必须由调用方处理，不能转成空正文。
type JavaV1PayloadCodec interface {
	Encode(interface{}) ([]byte, error)
	Decode([]byte) (interface{}, error)
}

const javaV1FrameMax = 8 * 1024 * 1024
const javaV1HeaderMax = 32767

func javaV1ReadString(data []byte, offset *int, nullable bool) (*string, error) {
	if len(data)-*offset < 2 {
		return nil, fmt.Errorf("truncated V1 header string length")
	}
	n := int(int16(binary.BigEndian.Uint16(data[*offset : *offset+2])))
	*offset += 2
	if n == -1 && nullable {
		return nil, nil
	}
	if n < 0 || n > len(data)-*offset {
		return nil, fmt.Errorf("invalid V1 header string length")
	}
	raw := data[*offset : *offset+n]
	*offset += n
	if !utf8.Valid(raw) {
		return nil, fmt.Errorf("invalid V1 header UTF-8")
	}
	value := string(raw)
	return &value, nil
}

func javaV1ReadHeaders(data []byte) ([]message.JavaV1HeaderEntry, map[string]string, error) {
	var entries []message.JavaV1HeaderEntry
	view := make(map[string]string)
	seen := make(map[string]bool)
	offset := 0
	for offset < len(data) {
		key, e := javaV1ReadString(data, &offset, false)
		if e != nil {
			return nil, nil, e
		}
		value, e := javaV1ReadString(data, &offset, true)
		if e != nil {
			return nil, nil, e
		}
		if seen[*key] {
			return nil, nil, fmt.Errorf("duplicate V1 header key")
		}
		seen[*key] = true
		entries = append(entries, message.JavaV1HeaderEntry{Key: *key, Value: value})
		if value != nil {
			view[*key] = *value
		}
	}
	return entries, view, nil
}

func (p *RpcPackageHandler) readJavaV1(data []byte) (interface{}, int, error) {
	// Getty会缓存未完整数据；在固定头到齐前既不解码，也不声明已消费字节。
	if len(data) < Seatav1HeaderLength {
		return nil, 0, nil
	}
	if data[0] != 0xda || data[1] != 0xda || data[2] != 1 {
		return nil, 0, fmt.Errorf("invalid Java V1 magic/version")
	}
	total := int(binary.BigEndian.Uint32(data[3:7]))
	head := int(binary.BigEndian.Uint16(data[7:9]))
	kind := data[9]
	if total < Seatav1HeaderLength || total > javaV1FrameMax || head < Seatav1HeaderLength || head > javaV1HeaderMax || head > total {
		return nil, 0, fmt.Errorf("invalid Java V1 frame lengths")
	}
	if kind > 4 || data[10] != 1 || data[11] != 0 {
		return nil, 0, fmt.Errorf("unsupported Java V1 kind/serializer/compressor")
	}
	if len(data) < total {
		return nil, 0, nil
	}
	entries, view, e := javaV1ReadHeaders(data[Seatav1HeaderLength:head])
	if e != nil {
		return nil, 0, e
	}
	rpc := message.RpcMessage{ID: int32(binary.BigEndian.Uint32(data[12:16])), Type: message.RequestType(kind), Codec: 1, Compressor: 0, HeadMap: view, HeaderEntries: entries}
	if kind >= 3 {
		if total != head {
			return nil, 0, fmt.Errorf("Java V1 heartbeat has a body")
		}
		rpc.Body = message.HeartBeatMessage{Ping: kind == 3}
	} else {
		if total == head {
			return nil, 0, fmt.Errorf("Java V1 ordinary message has no body")
		}
		// 只将当前帧正文交给codec；连续帧的后缀属于下一次调用。
		rpc.Body, e = p.PayloadCodec.Decode(data[head:total])
		if e != nil {
			return nil, 0, fmt.Errorf("Java V1 payload decode: %w", e)
		}
		if rpc.Body == nil {
			return nil, 0, fmt.Errorf("Java V1 payload decoded nil")
		}
	}
	return rpc, total, nil
}

func javaV1WriteHeaders(msg message.RpcMessage) ([]byte, error) {
	entries := msg.HeaderEntries
	if entries == nil {
		if len(msg.HeadMap) > (javaV1HeaderMax-Seatav1HeaderLength)/4 {
			return nil, fmt.Errorf("too many Java V1 headers")
		}
		keys := make([]string, 0, len(msg.HeadMap))
		for key := range msg.HeadMap {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			value := msg.HeadMap[key]
			entries = append(entries, message.JavaV1HeaderEntry{Key: key, Value: &value})
		}
	} else if len(entries) > (javaV1HeaderMax-Seatav1HeaderLength)/4 {
		return nil, fmt.Errorf("too many Java V1 headers")
	}
	seen := make(map[string]bool)
	nonnull := 0
	size := 0
	for _, entry := range entries {
		if seen[entry.Key] || !utf8.ValidString(entry.Key) || len(entry.Key) > 32767 {
			return nil, fmt.Errorf("invalid/duplicate Java V1 header key")
		}
		seen[entry.Key] = true
		size += 4 + len(entry.Key)
		if entry.Value != nil {
			if !utf8.ValidString(*entry.Value) || len(*entry.Value) > 32767 {
				return nil, fmt.Errorf("invalid Java V1 header value")
			}
			size += len(*entry.Value)
			nonnull++
			if msg.HeaderEntries != nil && msg.HeadMap != nil {
				value, present := msg.HeadMap[entry.Key]
				if !present || value != *entry.Value {
					return nil, fmt.Errorf("conflicting Java V1 header map")
				}
			}
		} else if msg.HeaderEntries != nil && msg.HeadMap != nil {
			if _, present := msg.HeadMap[entry.Key]; present {
				return nil, fmt.Errorf("null Java V1 header conflicts with map")
			}
		}
		if size > javaV1HeaderMax-Seatav1HeaderLength {
			return nil, fmt.Errorf("Java V1 header too large")
		}
	}
	if msg.HeaderEntries != nil && msg.HeadMap != nil && nonnull != len(msg.HeadMap) {
		return nil, fmt.Errorf("extra Java V1 header map keys")
	}
	out := make([]byte, 0, size)
	add := func(value string) {
		n := len(out)
		out = append(out, 0, 0)
		binary.BigEndian.PutUint16(out[n:n+2], uint16(len(value)))
		out = append(out, []byte(value)...)
	}
	for _, entry := range entries {
		add(entry.Key)
		if entry.Value == nil {
			out = append(out, 255, 255)
		} else {
			add(*entry.Value)
		}
	}
	return out, nil
}

func (p *RpcPackageHandler) writeJavaV1(pkg interface{}) ([]byte, error) {
	var msg message.RpcMessage
	switch value := pkg.(type) {
	case message.RpcMessage:
		msg = value
	case *message.RpcMessage:
		if value == nil {
			return nil, ErrInvalidPackage
		}
		msg = *value
	default:
		return nil, ErrInvalidPackage
	}
	if byte(msg.Type) > 4 || msg.Codec != 1 || msg.Compressor != 0 {
		return nil, fmt.Errorf("unsupported Java V1 kind/serializer/compressor")
	}
	header, e := javaV1WriteHeaders(msg)
	if e != nil {
		return nil, e
	}
	var body []byte
	if byte(msg.Type) >= 3 {
		expected := byte(msg.Type) == 3
		switch value := msg.Body.(type) {
		case nil:
		case message.HeartBeatMessage:
			if value.Ping != expected {
				return nil, fmt.Errorf("conflicting Java V1 heartbeat")
			}
		case *message.HeartBeatMessage:
			if value == nil || value.Ping != expected {
				return nil, fmt.Errorf("conflicting Java V1 heartbeat")
			}
		default:
			return nil, fmt.Errorf("unexpected Java V1 heartbeat body")
		}
	} else {
		body, e = p.PayloadCodec.Encode(msg.Body)
		if e != nil {
			return nil, fmt.Errorf("Java V1 payload encode: %w", e)
		}
		if len(body) == 0 {
			return nil, fmt.Errorf("Java V1 payload encoded empty")
		}
	}
	head := Seatav1HeaderLength + len(header)
	if len(body) > javaV1FrameMax-head {
		return nil, fmt.Errorf("Java V1 frame too large")
	}
	out := make([]byte, head+len(body))
	out[0] = 0xda
	out[1] = 0xda
	out[2] = 1
	binary.BigEndian.PutUint32(out[3:7], uint32(len(out)))
	binary.BigEndian.PutUint16(out[7:9], uint16(head))
	out[9] = byte(msg.Type)
	out[10] = 1
	binary.BigEndian.PutUint32(out[12:16], uint32(msg.ID))
	copy(out[16:], header)
	copy(out[head:], body)
	return out, nil
}
