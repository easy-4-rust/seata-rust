// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import (
	"fmt"
	"reflect"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"seata.apache.org/seata-go/v2/pkg/util/log"
)

// javaV1ReplyPlan 在发送前快照预期类型，不保留调用方可修改的子消息切片。
type javaV1ReplyPlan struct {
	merged bool
	types  []message.MessageType
}

var javaV1ReplyKinds = map[message.MessageType]message.MessageType{1: 2, 3: 4, 5: 6, 7: 8, 9: 10, 11: 12, 13: 14, 15: 16, 17: 18, 21: 22, 101: 102, 103: 104}

func javaV1SDKValue(body interface{}) (interface{}, bool) {
	v := reflect.ValueOf(body)
	if !v.IsValid() {
		return nil, false
	}
	if v.Kind() == reflect.Ptr {
		if v.IsNil() {
			return nil, false
		}
		v = v.Elem()
	}
	return v.Interface(), true
}
func javaV1SDKKind(body interface{}) (message.MessageType, bool) {
	value, ok := javaV1SDKValue(body)
	if !ok {
		return 0, false
	}
	aware, ok := value.(message.MessageTypeAware)
	if !ok {
		return 0, false
	}
	return aware.GetTypeCode(), true
}
func javaV1PlanFor(body interface{}) (*javaV1ReplyPlan, error) {
	value, ok := javaV1SDKValue(body)
	if !ok {
		return nil, nil
	}
	if request, ok := value.(message.MergedWarpMessage); ok {
		if len(request.Msgs) > 1024 || (len(request.MsgIds) != 0 && len(request.MsgIds) != len(request.Msgs)) {
			return nil, fmt.Errorf("invalid Java V1 merged request shape")
		}
		plan := &javaV1ReplyPlan{merged: true, types: make([]message.MessageType, len(request.Msgs))}
		for i, child := range request.Msgs {
			kind, known := javaV1SDKKind(child)
			expected, mapped := javaV1ReplyKinds[kind]
			if !known || !mapped {
				return nil, fmt.Errorf("unsupported Java V1 merged request child%d", i)
			}
			plan.types[i] = expected
		}
		return plan, nil
	}
	kind, known := javaV1SDKKind(value)
	if !known {
		return nil, nil
	}
	if expected, mapped := javaV1ReplyKinds[kind]; mapped {
		return &javaV1ReplyPlan{types: []message.MessageType{expected}}, nil
	}
	// 普通原有/单向消息沿用原SDK路径；合并子项才要求可确定的响应契约。
	return nil, nil
}
func (p *javaV1ReplyPlan) validate(body interface{}) error {
	if p == nil {
		return nil
	}
	value, ok := javaV1SDKValue(body)
	if !ok {
		return &JavaV1UnknownReplyError{Reason: "nil response"}
	}
	if p.merged {
		response, ok := value.(message.MergeResultMessage)
		if !ok || len(response.Msgs) != len(p.types) {
			return &JavaV1UnknownReplyError{Reason: "merged result count/type differs"}
		}
		for i, child := range response.Msgs {
			kind, ok := javaV1SDKKind(child)
			if !ok || kind != p.types[i] {
				return &JavaV1UnknownReplyError{Reason: fmt.Sprintf("merged result child%d differs", i)}
			}
		}
		return nil
	}
	kind, ok := javaV1SDKKind(value)
	if !ok || kind != p.types[0] {
		return &JavaV1UnknownReplyError{Reason: "response type differs"}
	}
	return nil
}
func (c *GettyRemotingClient) javaV1Callback(plan *javaV1ReplyPlan, async bool) callbackMethod {
	if plan == nil {
		if async {
			return c.asyncCallback
		}
		return c.syncCallback
	}
	await := func(req message.RpcMessage, future *message.MessageFuture) (interface{}, error) {
		response, e := c.syncCallback(req, future)
		if e != nil {
			return nil, e
		}
		if e = plan.validate(response); e != nil {
			return nil, e
		}
		return response, nil
	}
	if !async {
		return await
	}
	return func(req message.RpcMessage, future *message.MessageFuture) (interface{}, error) {
		go func() {
			if _, e := await(req, future); e != nil {
				log.Errorf("Java V1 async response: %v", e)
			}
		}()
		return nil, nil
	}
}

// HandleJavaV1MergedResponse 按外层future或旧逐子请求缓存分发60结果；不重发事务。
// 参数rpc必须是已解码的响应；返回值为关联/契约错误，错误时不部分完成子future。
func (c *GettyRemotingClient) HandleJavaV1MergedResponse(rpc message.RpcMessage) error {
	c.mergeReplyMutex.Lock()
	defer c.mergeReplyMutex.Unlock()
	request := c.claimJavaV1Merge(rpc.ID)
	value, ok := javaV1SDKValue(rpc.Body)
	if !ok {
		return c.failJavaV1Reply(rpc.ID, request, "nil merge response")
	}
	reply, ok := value.(message.MergeResultMessage)
	if !ok || rpc.Type != message.RequestTypeResponse {
		return c.failJavaV1Reply(rpc.ID, request, "not a merge response")
	}
	if request == nil {
		// 显式SendSyncRequest发送完整容器时，原SDK只创建外层future，没有逐子请求缓存。
		if c.GetMessageFuture(rpc.ID) != nil {
			c.NotifyRpcMessageResponse(rpc)
			c.RemoveMessageFuture(rpc.ID)
		}
		return nil
	}
	if len(request.MsgIds) != len(request.Msgs) {
		return c.failJavaV1Reply(rpc.ID, request, "merged request ID count differs")
	}
	seen := make(map[int32]bool, len(request.MsgIds))
	for _, id := range request.MsgIds {
		if seen[id] {
			return c.failJavaV1Reply(rpc.ID, request, "duplicate merged request ID")
		}
		seen[id] = true
	}
	plan, e := javaV1PlanFor(*request)
	if e != nil {
		return c.failJavaV1Reply(rpc.ID, request, e.Error())
	}
	if e = plan.validate(reply); e != nil {
		return c.failJavaV1Reply(rpc.ID, request, e.Error())
	}
	for i, id := range request.MsgIds {
		if c.GetMessageFuture(id) != nil {
			c.NotifyRpcMessageResponse(message.RpcMessage{ID: id, Type: 1, Body: reply.Msgs[i]})
			c.RemoveMessageFuture(id)
		}
	}
	return nil
}

func (c *GettyRemotingClient) claimJavaV1Merge(id int32) *message.MergedWarpMessage {
	value, ok := c.gettyRemoting.mergeMsgMap.LoadAndDelete(id)
	if !ok {
		return nil
	}
	request, _ := value.(*message.MergedWarpMessage)
	return request
}

// 调用方持有mergeReplyMutex；所有已知子项只发布同一个未知错误，绝不部分成功。
func (c *GettyRemotingClient) failJavaV1Reply(id int32, request *message.MergedWarpMessage, reason string) error {
	err := &JavaV1UnknownReplyError{Reason: reason}
	ids := []int32{id}
	if request != nil {
		ids = append(ids, request.MsgIds...)
	}
	seen := make(map[int32]bool)
	for _, child := range ids {
		if seen[child] {
			continue
		}
		seen[child] = true
		if future := c.GetMessageFuture(child); future != nil {
			future.CompleteWithError(nil, err)
			c.RemoveMessageFuture(child)
		}
	}
	return err
}

// RejectJavaV1Response 向对应等待者发布类型化未知错误，不修改事务状态或重发请求。
func (c *GettyRemotingClient) RejectJavaV1Response(id int32, reason string) error {
	c.mergeReplyMutex.Lock()
	defer c.mergeReplyMutex.Unlock()
	return c.failJavaV1Reply(id, c.claimJavaV1Merge(id), reason)
}
func (c *GettyRemotingClient) checkJavaV1Envelope(rpc message.RpcMessage) error {
	kind, known := javaV1SDKKind(rpc.Body)
	reply := known && (kind == 2 || kind == 4 || kind == 6 || kind == 8 || kind == 10 || kind == 12 || kind == 14 || kind == 16 || kind == 18 || kind == 22 || kind == 60 || kind == 102 || kind == 104)
	if (rpc.Type == message.RequestTypeResponse && !reply) || (rpc.Type != message.RequestTypeResponse && reply) {
		return c.RejectJavaV1Response(rpc.ID, "response envelope/body kind differs")
	}
	return nil
}
