// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty_test

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"reflect"
	"strings"
	"testing"

	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
)

func profileHandler(t *testing.T) *getty.RpcPackageHandler {
	t.Helper()
	codec.Init()
	p, e := codec.NewJavaV1MessageProfile(codec.GetCodecManager())
	if e != nil {
		t.Fatal(e)
	}
	return &getty.RpcPackageHandler{PayloadCodec: p}
}
func sampleRPC() message.RpcMessage {
	return message.RpcMessage{ID: -1, Type: message.RequestTypeRequestSync, Codec: 1, Body: message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: "xid"}}}
}
func str(s string) *string { return &s }
func fullFrame(body []byte, kind byte, id int32) []byte {
	wire := make([]byte, 16+len(body))
	wire[0] = 0xda
	wire[1] = 0xda
	wire[2] = 1
	binary.BigEndian.PutUint32(wire[3:7], uint32(len(wire)))
	binary.BigEndian.PutUint16(wire[7:9], 16)
	wire[9] = kind
	wire[10] = 1
	binary.BigEndian.PutUint32(wire[12:16], uint32(id))
	copy(wire[16:], body)
	return wire
}

func TestJavaV1GettyCapturedAndComposedFrames(t *testing.T) {
	h := profileHandler(t)
	codec.Init()
	p, e := codec.NewJavaV1MessageProfile(codec.GetCodecManager())
	if e != nil {
		t.Fatal(e)
	}
	raw, e := os.ReadFile(os.Getenv("SEATA_GETTY_VECTORS"))
	if e != nil {
		t.Fatal(e)
	}
	var rows []struct {
		Name   string `json:"name"`
		Frame  string `json:"frame_hex"`
		Source string `json:"source"`
	}
	if e = json.Unmarshal(raw, &rows); e != nil {
		t.Fatal(e)
	}
	if len(rows) != 326 {
		t.Fatalf("need9 Java frames+317 payload-composed frames, got%d", len(rows))
	}
	names := map[string]bool{}
	javaFrames := 0
	for _, r := range rows {
		t.Run(r.Name, func(t *testing.T) {
			if names[r.Name] {
				t.Fatal("duplicate")
			}
			names[r.Name] = true
			if r.Source == "java-frame" {
				javaFrames++
			}
			wire, e := hex.DecodeString(r.Frame)
			if e != nil {
				t.Fatal(e)
			}
			before := append([]byte(nil), wire...)
			val, n, e := h.Read(nil, wire)
			if e != nil || n != len(wire) {
				t.Fatalf("decode n=%d e=%v", n, e)
			}
			m := val.(message.RpcMessage)
			if m.ID != int32(binary.BigEndian.Uint32(wire[12:16])) || byte(m.Type) != wire[9] || m.Codec != 1 || m.Compressor != 0 {
				t.Fatal("correlation/header differs")
			}
			head := int(binary.BigEndian.Uint16(wire[7:9]))
			if wire[9] < 3 {
				expected, e := p.Decode(wire[head:])
				if e != nil || !reflect.DeepEqual(m.Body, expected) {
					t.Fatal("typed body differs", e)
				}
			} else if m.Body.(message.HeartBeatMessage).Ping != (wire[9] == 3) {
				t.Fatal("heartbeat flag differs")
			}
			out, e := h.Write(nil, m)
			if e != nil || !bytes.Equal(out, wire) {
				t.Fatal("bytes differ", e)
			}
			ptr, e := h.Write(nil, &m)
			if e != nil || !bytes.Equal(ptr, wire) {
				t.Fatal("pointer differs", e)
			}
			if !bytes.Equal(before, wire) {
				t.Fatal("input mutated")
			}
			// 所有有效前缀都必须等待，不可触发原生 codec 或消费半帧。
			for i := 0; i < len(wire); i++ {
				v, n, e := h.Read(nil, wire[:i])
				if v != nil || n != 0 || e != nil {
					t.Fatalf("prefix%d consumed/errored n=%d e=%v", i, n, e)
				}
			}
			twice := append(append([]byte(nil), wire...), wire...)
			v, n, e := h.Read(nil, twice)
			if e != nil || n != len(wire) || !reflect.DeepEqual(v, val) {
				t.Fatal("coalesced suffix contaminated first frame", e)
			}
		})
	}
	if javaFrames != 9 {
		t.Fatal("Java frame provenance lost")
	}
}

func TestJavaV1GettyHeaderOptionalityAndOrder(t *testing.T) {
	h := profileHandler(t)
	tests := [][]message.JavaV1HeaderEntry{
		{{Key: "", Value: str("")}}, {{Key: "null", Value: nil}}, {{Key: "z", Value: str("中文")}, {Key: "a", Value: str("")}},
		{{Key: "z", Value: nil}, {Key: "", Value: str("")}, {Key: "a", Value: str("x")}}, {{Key: "key", Value: str(strings.Repeat("x", 32744))}},
	}
	for i, entries := range tests {
		m := sampleRPC()
		m.HeaderEntries = entries
		original, e := json.Marshal(m)
		if e != nil {
			t.Fatal(e)
		}
		wire, e := h.Write(nil, m)
		if e != nil {
			t.Fatal(i, e)
		}
		if i == 4 && binary.BigEndian.Uint16(wire[7:9]) != 32767 {
			t.Fatal("maximum header not exact")
		}
		v, n, e := h.Read(nil, wire)
		if e != nil || n != len(wire) {
			t.Fatal(i, e)
		}
		out := v.(message.RpcMessage)
		if !reflect.DeepEqual(out.HeaderEntries, entries) {
			t.Fatalf("null/empty/order changed: %#v", out.HeaderEntries)
		}
		for _, entry := range entries {
			value, present := out.HeadMap[entry.Key]
			if (entry.Value == nil && present) || (entry.Value != nil && (!present || value != *entry.Value)) {
				t.Fatal("nonnull map view differs")
			}
		}
		re, e := h.Write(nil, out)
		if e != nil || !bytes.Equal(re, wire) {
			t.Fatal("header bytes not preserved", e)
		}
		after, _ := json.Marshal(m)
		if !bytes.Equal(original, after) {
			t.Fatal("caller mutated")
		}
	}
	m := sampleRPC()
	m.HeadMap = map[string]string{"z": "1", "a": "2"}
	first, e := h.Write(nil, m)
	if e != nil {
		t.Fatal(e)
	}
	for i := 0; i < 30; i++ {
		next, e := h.Write(nil, m)
		if e != nil || !bytes.Equal(first, next) {
			t.Fatal("map producer ordering unstable")
		}
	}
	v, _, e := h.Read(nil, first)
	if e != nil || v.(message.RpcMessage).HeaderEntries[0].Key != "a" {
		t.Fatal("map producer not canonical")
	}
}

func TestJavaV1GettyRejectsMalformedFrames(t *testing.T) {
	h := profileHandler(t)
	base, e := h.Write(nil, sampleRPC())
	if e != nil {
		t.Fatal(e)
	}
	cases := map[string][]byte{}
	put := func(name string, f func([]byte)) { b := append([]byte(nil), base...); f(b); cases[name] = b }
	put("magic", func(b []byte) { b[0] = 0 })
	put("version", func(b []byte) { b[2] = 2 })
	put("shortTotal", func(b []byte) { binary.BigEndian.PutUint32(b[3:7], 15) })
	put("largeTotal", func(b []byte) { binary.BigEndian.PutUint32(b[3:7], 8*1024*1024+1) })
	put("shortHead", func(b []byte) { binary.BigEndian.PutUint16(b[7:9], 15) })
	put("headBeyondTotal", func(b []byte) { binary.BigEndian.PutUint16(b[7:9], uint16(len(b)+1)) })
	put("largeHead", func(b []byte) { binary.BigEndian.PutUint16(b[7:9], 32768) })
	put("kind", func(b []byte) { b[9] = 5 })
	put("codec", func(b []byte) { b[10] = 2 })
	put("compressor", func(b []byte) { b[11] = 1 })
	put("badPayload", func(b []byte) { b[16] = 0xff })
	cases["emptyNormal"] = fullFrame(nil, 0, 1)
	cases["heartbeatBody"] = fullFrame([]byte{0}, 3, 1)
	for name, header := range map[string][]byte{"nullKey": {255, 255, 0, 0}, "badValueLength": {0, 0, 255, 254}, "truncatedString": {0, 4, 'a'}, "invalidKeyUtf8": {0, 1, 255, 0, 0}, "invalidValueUtf8": {0, 0, 0, 1, 255}, "duplicateKey": {0, 0, 0, 0, 0, 0, 0, 0}, "halfLength": {0}} {
		b := fullFrame(header, 3, 1)
		binary.BigEndian.PutUint16(b[7:9], uint16(len(b)))
		cases[name] = b
	}
	for name, b := range cases {
		t.Run(name, func(t *testing.T) {
			v, n, e := h.Read(nil, b)
			if e == nil || v != nil || n != 0 {
				t.Fatalf("malformed accepted n=%d err=%v", n, e)
			}
		})
	}
	// 对合法小帧逐字节变异：若接受，则解码/编码必须保持完整字节。
	for i := range base {
		for _, x := range []byte{0, 1, 127, 255} {
			b := append([]byte(nil), base...)
			b[i] = x
			v, n, e := h.Read(nil, b)
			if e == nil && v != nil {
				re, e := h.Write(nil, v)
				if e != nil || n > len(b) || !bytes.Equal(re, b[:n]) {
					t.Fatalf("accepted lossy mutation%d", i)
				}
			}
		}
	}
}

func TestJavaV1GettyRejectsLossyWrites(t *testing.T) {
	h := profileHandler(t)
	cases := []interface{}{nil, (*message.RpcMessage)(nil), "wrong"}
	add := func(f func(*message.RpcMessage)) { m := sampleRPC(); f(&m); cases = append(cases, m) }
	add(func(m *message.RpcMessage) { m.Codec = 0 })
	add(func(m *message.RpcMessage) { m.Compressor = 1 })
	add(func(m *message.RpcMessage) { m.Type = 5 })
	add(func(m *message.RpcMessage) { m.Body = nil })
	add(func(m *message.RpcMessage) {
		m.HeaderEntries = []message.JavaV1HeaderEntry{{Key: "x", Value: nil}}
		m.HeadMap = map[string]string{"x": ""}
	})
	add(func(m *message.RpcMessage) {
		m.HeaderEntries = []message.JavaV1HeaderEntry{{Key: "x", Value: str("a")}, {Key: "x", Value: str("b")}}
	})
	add(func(m *message.RpcMessage) {
		m.HeaderEntries = []message.JavaV1HeaderEntry{}
		m.HeadMap = map[string]string{"x": "x"}
	})
	add(func(m *message.RpcMessage) { m.HeadMap = map[string]string{"x": strings.Repeat("x", 32768)} })
	add(func(m *message.RpcMessage) { m.HeadMap = map[string]string{string([]byte{255}): "x"} })
	add(func(m *message.RpcMessage) { m.HeadMap = map[string]string{"x": string([]byte{255})} })
	add(func(m *message.RpcMessage) { m.HeaderEntries = make([]message.JavaV1HeaderEntry, 8190) })
	add(func(m *message.RpcMessage) { m.Type = 3; m.Body = message.HeartBeatMessagePong })
	add(func(m *message.RpcMessage) { m.Type = 4; m.Body = message.HeartBeatMessagePing })
	add(func(m *message.RpcMessage) { m.Type = 3 })
	for i, m := range cases {
		if b, e := h.Write(nil, m); e == nil || b != nil {
			t.Fatalf("lossy write%d accepted", i)
		}
	}
	for _, kind := range []message.RequestType{3, 4} {
		for _, body := range []interface{}{nil, message.HeartBeatMessage{Ping: kind == 3}, &message.HeartBeatMessage{Ping: kind == 3}} {
			m := sampleRPC()
			m.Type = kind
			m.Body = body
			if _, e := h.Write(nil, m); e != nil {
				t.Fatal(e)
			}
		}
	}
}

type failedPayload struct{ cause error }

func (p failedPayload) Encode(interface{}) ([]byte, error) { return nil, p.cause }
func (p failedPayload) Decode([]byte) (interface{}, error) { return nil, p.cause }
func TestJavaV1GettyPropagatesPayloadErrors(t *testing.T) {
	cause := errors.New("payload sentinel")
	h := &getty.RpcPackageHandler{PayloadCodec: failedPayload{cause}}
	if _, e := h.Write(nil, sampleRPC()); !errors.Is(e, cause) {
		t.Fatal("encode cause lost", e)
	}
	if _, _, e := h.Read(nil, fullFrame([]byte{0, 15, 0, 1, 'x', 0, 0}, 0, 1)); !errors.Is(e, cause) {
		t.Fatal("decode cause lost", e)
	}
	var p *codec.JavaV1MessageProfile
	h.PayloadCodec = p
	if _, e := h.Write(nil, sampleRPC()); e == nil {
		t.Fatal("typed nil codec accepted")
	}
	if _, _, e := h.Read(nil, fullFrame([]byte{0, 15, 0, 1, 'x', 0, 0}, 0, 1)); e == nil {
		t.Fatal("typed nil decode accepted")
	}
}

func TestJavaV1GettyDefaultAndExplicitSelection(t *testing.T) {
	h := profileHandler(t)
	native := &getty.RpcPackageHandler{}
	m := sampleRPC()
	m.Compressor = 1
	if _, e := native.Write(nil, m); e != nil {
		t.Fatal("existing default changed", e)
	}
	if _, e := h.Write(nil, m); e == nil {
		t.Fatal("explicit mode silently ignored compressor")
	}
	m = sampleRPC()
	m.HeaderEntries = []message.JavaV1HeaderEntry{{Key: "x", Value: nil}}
	if _, e := native.Write(nil, m); e == nil {
		t.Fatal("default silently discarded new metadata")
	}
	if codec.GetCodecManager().GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMerge) != nil {
		t.Fatal("default registry changed")
	}
	for _, id := range []int32{0, -1, -2147483648, 2147483647} {
		for _, kind := range []message.RequestType{0, 1, 2} {
			m = sampleRPC()
			m.ID = id
			m.Type = kind
			b, e := h.Write(nil, m)
			if e != nil {
				t.Fatal(e)
			}
			v, _, e := h.Read(nil, b)
			if e != nil || v.(message.RpcMessage).ID != id || v.(message.RpcMessage).Type != kind {
				t.Fatal("signed id/kind differs", e)
			}
		}
	}
}

func TestJavaV1GettyFrameSizeBoundary(t *testing.T) {
	h := profileHandler(t)
	m := sampleRPC()
	// type/XID/branchId/branchType/resourceId/ApplicationData前缀共21字节，固定头16字节。
	m.Body = message.BranchCommitRequest{AbstractBranchEndRequest: message.AbstractBranchEndRequest{Xid: "x", ResourceId: "r", ApplicationData: make([]byte, 8*1024*1024-37)}}
	b, e := h.Write(nil, m)
	if e != nil || len(b) != 8*1024*1024 {
		t.Fatal("exact frame boundary", len(b), e)
	}
	v, n, e := h.Read(nil, b)
	if e != nil || n != len(b) || !reflect.DeepEqual(v.(message.RpcMessage).Body, m.Body) {
		t.Fatal("maximum frame decode", e)
	}
	m.Body = message.BranchCommitRequest{AbstractBranchEndRequest: message.AbstractBranchEndRequest{Xid: "x", ResourceId: "r", ApplicationData: make([]byte, 8*1024*1024-36)}}
	if b, e := h.Write(nil, m); e == nil || b != nil {
		t.Fatal("oversize frame accepted")
	}
	m = sampleRPC()
	m.HeadMap = map[string]string{"key": strings.Repeat("x", 32745)}
	if b, e := h.Write(nil, m); e == nil || b != nil {
		t.Fatal("oversize aggregate header accepted")
	}
}
