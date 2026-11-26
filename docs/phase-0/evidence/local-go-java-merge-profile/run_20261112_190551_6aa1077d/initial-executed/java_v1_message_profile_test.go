// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package codec_test

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"os"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"strings"
	"testing"
	"time"
)

func mergeProfile(t *testing.T) *codec.JavaV1MessageProfile {
	t.Helper()
	codec.Init()
	p, err := codec.NewJavaV1MessageProfile(codec.GetCodecManager())
	if err != nil {
		t.Fatal(err)
	}
	return p
}
func mergeLeaf() message.MessageTypeAware {
	return message.GlobalBeginRequest{Timeout: 2000 * time.Millisecond, TransactionName: "leaf"}
}

func TestJavaV1MessageProfilePreservesSignedTimeout(t *testing.T) {
	p := mergeProfile(t)
	for _, millis := range []int64{-1, -2147483648, 0, 2147483647} {
		wire := []byte{0, 1, 0, 0, 0, 0, 0, 0}
		binary.BigEndian.PutUint32(wire[2:6], uint32(int32(millis)))
		value, err := p.Decode(wire)
		if err != nil {
			t.Fatal(err)
		}
		if value.(message.GlobalBeginRequest).Timeout != time.Duration(millis)*time.Millisecond {
			t.Errorf("signed timeout%d changed to%d", millis, value.(message.GlobalBeginRequest).Timeout/time.Millisecond)
		}
	}
	if _, err := p.Encode(message.GlobalBeginRequest{Timeout: 2147483648 * time.Millisecond}); err == nil {
		t.Error("positive timeout became negative on Java wire")
	}
}
func TestJavaV1MergeProfileRejectsEveryTruncationAndBadShape(t *testing.T) {
	p := mergeProfile(t)
	raw, err := os.ReadFile(os.Getenv("SEATA_MERGE_VECTORS"))
	if err != nil {
		t.Fatal(err)
	}
	var rows []struct {
		Name string `json:"name"`
		Hex  string `json:"serialized_hex"`
	}
	if err = json.Unmarshal(raw, &rows); err != nil {
		t.Fatal(err)
	}
	if len(rows) != 30 {
		t.Fatal("need30 Java merge records")
	}
	for _, row := range rows {
		wire, err := hex.DecodeString(row.Hex)
		if err != nil {
			t.Fatal(err)
		}
		for n := 0; n < len(wire); n++ {
			if v, err := p.Decode(wire[:n]); err == nil || v != nil {
				t.Fatalf("%s prefix%d accepted", row.Name, n)
			}
		}
		bad := append(append([]byte(nil), wire...), 0)
		if _, err := p.Decode(bad); err == nil {
			t.Fatal("trailing byte accepted")
		}
		for _, change := range []func([]byte){func(b []byte) { b[2] = 0x80 }, func(b []byte) { b[6] = 0xff; b[7] = 0xff }, func(b []byte) { b[6] = 4; b[7] = 1 }, func(b []byte) { b[8] = 0xff; b[9] = 0xff }} {
			bad := append([]byte(nil), wire...)
			change(bad)
			if _, err := p.Decode(bad); err == nil {
				t.Fatal("invalid sealed length/count/type accepted")
			}
		}
		if row.Name == "All25WithIds" {
			bad := append([]byte(nil), wire[:len(wire)-1]...)
			binary.BigEndian.PutUint32(bad[2:6], uint32(len(bad)-6))
			if _, err := p.Decode(bad); err == nil {
				t.Fatal("partial request IDs accepted")
			}
		}
	}
	bad := []byte{0, 60, 0, 0, 0, 8, 0, 1, 0, 1, 0, 0, 0, 0}
	if _, err := p.Decode(bad); err == nil {
		t.Fatal("request inside result accepted")
	}
}
func TestJavaV1MergeProfileSymmetricDepthAndNodeLimits(t *testing.T) {
	p := mergeProfile(t)
	var nested message.MessageTypeAware = mergeLeaf()
	for i := 0; i < 8; i++ {
		nested = message.MergedWarpMessage{Msgs: []message.MessageTypeAware{nested}}
	}
	wire, err := p.Encode(nested)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := p.Decode(wire); err != nil {
		t.Fatal(err)
	}
	nested = message.MergedWarpMessage{Msgs: []message.MessageTypeAware{nested}}
	if _, err := p.Encode(nested); err == nil {
		t.Fatal("encode depth limit missing")
	}
	content := append([]byte{0, 1}, wire...)
	deep := append([]byte{0, 59, 0, 0, 0, 0}, content...)
	binary.BigEndian.PutUint32(deep[2:6], uint32(len(content)))
	if _, err := p.Decode(deep); err == nil {
		t.Fatal("decode depth limit missing")
	}
	group := func(count int) message.MergedWarpMessage {
		g := message.MergedWarpMessage{Msgs: make([]message.MessageTypeAware, count)}
		for i := range g.Msgs {
			g.Msgs[i] = mergeLeaf()
		}
		return g
	}
	valid := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{group(1022), group(1022)}}
	wire, err = p.Encode(valid)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := p.Decode(wire); err != nil {
		t.Fatal(err)
	}
	invalid := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{group(1023), group(1023)}}
	if _, err := p.Encode(invalid); err == nil {
		t.Fatal("total node limit missing")
	}
	left, _ := p.Encode(group(1023))
	right, _ := p.Encode(group(1023))
	body := append(append([]byte{0, 2}, left...), right...)
	bad := append([]byte{0, 59, 0, 0, 0, 0}, body...)
	binary.BigEndian.PutUint32(bad[2:6], uint32(len(body)))
	if _, err := p.Decode(bad); err == nil {
		t.Fatal("decode total limit missing")
	}
	if _, err := p.Encode(group(1025)); err == nil {
		t.Fatal("per-container limit missing")
	}
	cycle := &message.MergedWarpMessage{}
	cycle.Msgs = []message.MessageTypeAware{cycle}
	if _, err := p.Encode(cycle); err == nil {
		t.Fatal("cycle accepted")
	}
}
func TestJavaV1MergeProfileRejectsLossyInputsAndUnknownObjects(t *testing.T) {
	p := mergeProfile(t)
	var missing *message.MergedWarpMessage
	for _, v := range []interface{}{nil, missing, message.MergedWarpMessage{Msgs: []message.MessageTypeAware{nil}}, message.MergedWarpMessage{Msgs: []message.MessageTypeAware{mergeLeaf()}, MsgIds: []int32{1, 2}}, message.MergeResultMessage{Msgs: []message.MessageTypeAware{mergeLeaf()}}} {
		if _, err := p.Encode(v); err == nil {
			t.Fatal("invalid input accepted")
		}
	}
	if _, err := p.Encode(message.GlobalBeginRequest{Timeout: time.Nanosecond, TransactionName: "x"}); err == nil {
		t.Fatal("sub-millisecond timeout silently truncated")
	}
	if _, err := p.Encode(message.GlobalBeginRequest{TransactionName: strings.Repeat("x", 32768)}); err == nil {
		t.Fatal("oversized name silently truncated")
	}
	bad := message.RegisterTMResponse{AbstractIdentifyResponse: message.AbstractIdentifyResponse{AbstractResultMessage: message.AbstractResultMessage{Msg: "not serialized"}}}
	if _, err := p.Encode(bad); err == nil {
		t.Fatal("ignored registration field silently lost")
	}
	if _, err := p.Decode(make([]byte, 8*1024*1024+1)); err == nil {
		t.Fatal("payload limit missing")
	}
	zero := &codec.JavaV1MessageProfile{}
	if _, err := zero.Encode(mergeLeaf()); err == nil {
		t.Fatal("zero profile accepted")
	}
	if _, err := zero.Decode([]byte{0, 14, 1, 0}); err == nil {
		t.Fatal("zero decode profile accepted")
	}
}
func TestJavaV1MergeProfileMutationsRejectOrReencodeExactly(t *testing.T) {
	p := mergeProfile(t)
	value := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{mergeLeaf()}, MsgIds: []int32{-7}}
	wire, err := p.Encode(value)
	if err != nil {
		t.Fatal(err)
	}
	for at := range wire {
		for replacement := 0; replacement < 256; replacement++ {
			bad := append([]byte(nil), wire...)
			bad[at] = byte(replacement)
			decoded, err := p.Decode(bad)
			if err != nil {
				continue
			}
			encoded, err := p.Encode(decoded)
			if err != nil || !bytes.Equal(encoded, bad) {
				t.Fatalf("accepted mutation not stable: at%d value%d", at, replacement)
			}
		}
	}
	if codec.GetCodecManager().GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMerge) != nil {
		t.Fatal("explicit profile changed native registry")
	}
}
