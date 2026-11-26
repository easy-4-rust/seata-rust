// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package codec

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"reflect"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"time"
	"unicode/utf8"
)

// JavaV1MessageProfile 显式处理25类普通消息与59/60容器；对应Java: SeataSerializerV1。
// 不修改默认registry；不是TCP帧或Getty连接实现。
// 注册响应的原生Version仅为原始线缆字段；Java getter语义必须通过RegistrationVersion及明确的Java版本上下文取得。
type JavaV1MessageProfile struct {
	manager   *CodecManager
	responses *JavaV1ResponseProfile
}

const javaV1MessageMax = 8 * 1024 * 1024

var javaV1NativeTypes = map[reflect.Type]uint16{
	reflect.TypeOf(message.GlobalBeginRequest{}): 1, reflect.TypeOf(message.GlobalBeginResponse{}): 2, reflect.TypeOf(message.BranchCommitRequest{}): 3, reflect.TypeOf(message.BranchCommitResponse{}): 4,
	reflect.TypeOf(message.BranchRollbackRequest{}): 5, reflect.TypeOf(message.BranchRollbackResponse{}): 6, reflect.TypeOf(message.GlobalCommitRequest{}): 7, reflect.TypeOf(message.GlobalCommitResponse{}): 8,
	reflect.TypeOf(message.GlobalRollbackRequest{}): 9, reflect.TypeOf(message.GlobalRollbackResponse{}): 10, reflect.TypeOf(message.BranchRegisterRequest{}): 11, reflect.TypeOf(message.BranchRegisterResponse{}): 12,
	reflect.TypeOf(message.BranchReportRequest{}): 13, reflect.TypeOf(message.BranchReportResponse{}): 14, reflect.TypeOf(message.GlobalStatusRequest{}): 15, reflect.TypeOf(message.GlobalStatusResponse{}): 16,
	reflect.TypeOf(message.GlobalReportRequest{}): 17, reflect.TypeOf(message.GlobalReportResponse{}): 18, reflect.TypeOf(message.GlobalLockQueryRequest{}): 21, reflect.TypeOf(message.GlobalLockQueryResponse{}): 22,
	reflect.TypeOf(message.RegisterTMRequest{}): 101, reflect.TypeOf(message.RegisterTMResponse{}): 102, reflect.TypeOf(message.RegisterRMRequest{}): 103, reflect.TypeOf(message.RegisterRMResponse{}): 104, reflect.TypeOf(message.UndoLogDeleteRequest{}): 111,
	reflect.TypeOf(message.MergedWarpMessage{}): 59, reflect.TypeOf(message.MergeResultMessage{}): 60,
}

// NewJavaV1MessageProfile 使用稳定的已初始化registry创建入口；依赖0001/0002，nil返回错误。
func NewJavaV1MessageProfile(manager *CodecManager) (*JavaV1MessageProfile, error) {
	p, e := NewJavaV1ResponseProfile(manager)
	if e != nil {
		return nil, e
	}
	return &JavaV1MessageProfile{manager: manager, responses: p}, nil
}
func javaV1Visit(depth int, budget *int) error {
	if depth > 8 || *budget <= 0 {
		return fmt.Errorf("V1 depth/node budget exceeded")
	}
	*budget--
	return nil
}
func javaV1Value(in interface{}) (interface{}, uint16, error) {
	v := reflect.ValueOf(in)
	if !v.IsValid() {
		return nil, 0, fmt.Errorf("nil V1 message")
	}
	if v.Kind() == reflect.Ptr {
		if v.IsNil() {
			return nil, 0, fmt.Errorf("nil V1 message pointer")
		}
		v = v.Elem()
	}
	kind, ok := javaV1NativeTypes[v.Type()]
	if !ok {
		return nil, 0, fmt.Errorf("unsupported V1 model")
	}
	return v.Interface(), kind, nil
}

// 原生Go模型中的零字节切片nil/empty均没有线缆区别；不对字符串/版本做通用缺省处理。
func javaV1Canonical(v reflect.Value) reflect.Value {
	if v.Kind() == reflect.Slice && v.Type().Elem().Kind() == reflect.Uint8 && v.Len() == 0 {
		return reflect.MakeSlice(v.Type(), 0, 0)
	}
	if v.Kind() != reflect.Struct {
		return v
	}
	out := reflect.New(v.Type()).Elem()
	for i := 0; i < v.NumField(); i++ {
		out.Field(i).Set(javaV1Canonical(v.Field(i)))
	}
	return out
}
func javaV1InputSize(v reflect.Value, total *int) error {
	switch v.Kind() {
	case reflect.Struct:
		for i := 0; i < v.NumField(); i++ {
			if e := javaV1InputSize(v.Field(i), total); e != nil {
				return e
			}
		}
	case reflect.String:
		if !utf8.ValidString(v.String()) {
			return fmt.Errorf("invalid input UTF8")
		}
		*total += v.Len()
	case reflect.Slice:
		if v.Type().Elem().Kind() != reflect.Uint8 {
			return fmt.Errorf("unexpected ordinary slice")
		}
		if !utf8.Valid(v.Bytes()) {
			return fmt.Errorf("invalid input UTF8 bytes")
		}
		*total += v.Len()
	}
	if *total > javaV1MessageMax {
		return fmt.Errorf("ordinary input size exceeded")
	}
	return nil
}
func (p *JavaV1MessageProfile) plainEncode(value interface{}, kind uint16) ([]byte, error) {
	total := 0
	if e := javaV1InputSize(reflect.ValueOf(value), &total); e != nil {
		return nil, e
	}
	if p.manager.GetCodec(CodecTypeSeata, message.MessageType(kind)) == nil {
		return nil, fmt.Errorf("missing native V1 codec")
	}
	var wire []byte
	var e error
	if javaV1TransactionKind(kind) {
		wire, e = p.responses.Encode(value)
	} else {
		wire = p.manager.Encode(CodecTypeSeata, value)
	}
	if e != nil {
		return nil, e
	}
	if len(wire) > javaV1MessageMax {
		return nil, fmt.Errorf("payload limit exceeded")
	}
	c := javaV1MessageCursor{data: wire}
	actual, e := c.u16()
	if e != nil || actual != kind {
		return nil, fmt.Errorf("wrong native V1 type")
	}
	if e := c.ordinary(kind); e != nil {
		return nil, e
	}
	if c.at != len(wire) {
		return nil, fmt.Errorf("native V1 trailing data")
	}
	back, e := p.plainDecode(wire, kind)
	if e != nil {
		return nil, e
	}
	if !reflect.DeepEqual(javaV1Canonical(reflect.ValueOf(value)).Interface(), javaV1Canonical(reflect.ValueOf(back)).Interface()) {
		return nil, fmt.Errorf("native codec cannot preserve input fields")
	}
	return wire, nil
}
func (p *JavaV1MessageProfile) plainDecode(wire []byte, kind uint16) (message.MessageTypeAware, error) {
	if p.manager.GetCodec(CodecTypeSeata, message.MessageType(kind)) == nil {
		return nil, fmt.Errorf("missing native V1 codec")
	}
	var value interface{}
	var e error
	if javaV1TransactionKind(kind) {
		value, e = p.responses.Decode(wire)
	} else {
		value = p.manager.Decode(CodecTypeSeata, wire)
	}
	if e != nil {
		return nil, e
	}
	value, actual, e := javaV1Value(value)
	if e != nil || actual != kind {
		return nil, fmt.Errorf("wrong native decoded type")
	}
	if kind == 1 {
		// Go原生按uint32读取；Java字段是有符号int，不能仅凭相同字节认定语义相同。
		request := value.(message.GlobalBeginRequest)
		request.Timeout = time.Duration(int32(binary.BigEndian.Uint32(wire[2:6]))) * time.Millisecond
		value = request
	}
	var back []byte
	if javaV1TransactionKind(kind) {
		back, e = p.responses.Encode(value)
	} else {
		back = p.manager.Encode(CodecTypeSeata, value)
	}
	if e != nil || !bytes.Equal(back, wire) {
		return nil, fmt.Errorf("lossy native V1 decode")
	}
	return value.(message.MessageTypeAware), nil
}
func javaV1Append(out, child []byte) ([]byte, error) {
	if len(child) > javaV1MessageMax-len(out) {
		return nil, fmt.Errorf("merged payload size exceeded")
	}
	return append(out, child...), nil
}
func (p *JavaV1MessageProfile) write(in interface{}, depth int, budget *int) ([]byte, error) {
	if e := javaV1Visit(depth, budget); e != nil {
		return nil, e
	}
	value, kind, e := javaV1Value(in)
	if e != nil {
		return nil, e
	}
	if kind != 59 && kind != 60 {
		return p.plainEncode(value, kind)
	}
	var children []message.MessageTypeAware
	var ids []int32
	if kind == 59 {
		v := value.(message.MergedWarpMessage)
		children = v.Msgs
		ids = v.MsgIds
		if len(ids) != 0 && len(ids) != len(children) {
			return nil, fmt.Errorf("partial/extra merge IDs")
		}
	} else {
		children = value.(message.MergeResultMessage).Msgs
	}
	if len(children) > 1024 {
		return nil, fmt.Errorf("merge child count exceeded")
	}
	content := make([]byte, 2)
	binary.BigEndian.PutUint16(content, uint16(len(children)))
	for _, child := range children {
		if kind == 60 {
			_, code, e := javaV1Value(child)
			if e != nil || !javaV1ResultKind(code) {
				return nil, fmt.Errorf("non-result inside merge result")
			}
		}
		data, e := p.write(child, depth+1, budget)
		if e != nil {
			return nil, e
		}
		content, e = javaV1Append(content, data)
		if e != nil {
			return nil, e
		}
	}
	for _, id := range ids {
		tail := make([]byte, 4)
		binary.BigEndian.PutUint32(tail, uint32(id))
		content, e = javaV1Append(content, tail)
		if e != nil {
			return nil, e
		}
	}
	wire := make([]byte, 6)
	binary.BigEndian.PutUint16(wire, kind)
	binary.BigEndian.PutUint32(wire[2:], uint32(len(content)))
	return javaV1Append(wire, content)
}
func (p *JavaV1MessageProfile) read(c *javaV1MessageCursor, depth int, budget *int) (message.MessageTypeAware, error) {
	if e := javaV1Visit(depth, budget); e != nil {
		return nil, e
	}
	start := c.at
	kind, e := c.u16()
	if e != nil {
		return nil, e
	}
	if kind != 59 && kind != 60 {
		if e := c.ordinary(kind); e != nil {
			return nil, e
		}
		return p.plainDecode(c.data[start:c.at], kind)
	}
	length, e := c.take(4)
	if e != nil {
		return nil, e
	}
	n := int(int32(binary.BigEndian.Uint32(length)))
	if n < 2 {
		return nil, fmt.Errorf("invalid merge content length")
	}
	data, e := c.take(n)
	if e != nil {
		return nil, e
	}
	sub := javaV1MessageCursor{data: data}
	countRaw, e := sub.u16()
	if e != nil {
		return nil, e
	}
	count := int(int16(countRaw))
	if count < 0 || count > 1024 {
		return nil, fmt.Errorf("invalid merge child count")
	}
	children := make([]message.MessageTypeAware, 0, count)
	for i := 0; i < count; i++ {
		if kind == 60 {
			if len(sub.data)-sub.at < 2 || !javaV1ResultKind(binary.BigEndian.Uint16(sub.data[sub.at:])) {
				return nil, fmt.Errorf("non-result in merge result")
			}
		}
		child, e := p.read(&sub, depth+1, budget)
		if e != nil {
			return nil, e
		}
		children = append(children, child)
	}
	var ids []int32
	if kind == 59 && sub.at != len(sub.data) {
		if len(sub.data)-sub.at != count*4 {
			return nil, fmt.Errorf("partial/extra merged IDs")
		}
		ids = make([]int32, count)
		for i := range ids {
			value, e := sub.take(4)
			if e != nil {
				return nil, e
			}
			ids[i] = int32(binary.BigEndian.Uint32(value))
		}
	}
	if sub.at != len(sub.data) {
		return nil, fmt.Errorf("trailing merge content")
	}
	if kind == 59 {
		return message.MergedWarpMessage{Msgs: children, MsgIds: ids}, nil
	}
	return message.MergeResultMessage{Msgs: children}, nil
}

// Encode 复制并严格校验普通/合并模型，返回新payload或错误；两方向均限制深度和总节点。
func (p *JavaV1MessageProfile) Encode(in interface{}) ([]byte, error) {
	if p == nil || p.manager == nil || p.responses == nil {
		return nil, fmt.Errorf("uninitialized V1 message profile")
	}
	budget := 2048
	return p.write(in, 0, &budget)
}

// Decode 输入完整序列化payload，返回类型对象；未知/截断/尾随/资源预算错误均拒绝。
func (p *JavaV1MessageProfile) Decode(wire []byte) (interface{}, error) {
	if p == nil || p.manager == nil || p.responses == nil {
		return nil, fmt.Errorf("uninitialized V1 message profile")
	}
	if len(wire) > javaV1MessageMax {
		return nil, fmt.Errorf("V1 payload size exceeded")
	}
	c := javaV1MessageCursor{data: wire}
	budget := 2048
	value, e := p.read(&c, 0, &budget)
	if e != nil {
		return nil, e
	}
	if c.at != len(wire) {
		return nil, fmt.Errorf("trailing V1 payload")
	}
	return value, nil
}
