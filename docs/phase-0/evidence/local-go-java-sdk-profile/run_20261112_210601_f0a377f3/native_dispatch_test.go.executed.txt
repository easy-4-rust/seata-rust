// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty_test

import (
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"seata.apache.org/seata-go/v2/pkg/remoting/config"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
	client "seata.apache.org/seata-go/v2/pkg/remoting/processor/client"
	"testing"
)

func nativeDispatch(reply interface{}, id int32) {
	config.InitTransportConfig(&config.TransportConfig{Protocol: "seata"})
	client.RegisterProcessor()
	getty.GetGettyClientHandlerInstance().OnMessage(nil, message.RpcMessage{ID: id, Type: 1, Codec: 1, Body: reply})
}
func nativeRequest() message.MessageTypeAware { return message.GlobalStatusRequest{} }
func nativeReply() message.MessageTypeAware   { return message.GlobalStatusResponse{} }
func TestJavaV1SDKBaselineWholeReply(t *testing.T) {
	id := int32(-22000)
	f := getty.Phase0NativeFuture(id)
	defer getty.GetGettyRemotingClient().RemoveMessageFuture(id)
	nativeDispatch(message.MergeResultMessage{Msgs: []message.MessageTypeAware{nativeReply()}}, id)
	select {
	case <-f.Done:
	default:
		t.Fatal("SDK RED actual processor dropped whole merged reply")
	}
}
func TestJavaV1SDKBaselineMalformedLegacyReply(t *testing.T) {
	id := int32(-22100)
	child := int32(-22101)
	other := int32(-22102)
	c := getty.GetGettyRemotingClient()
	defer c.RemoveMergedMessageFuture(id)
	defer c.RemoveMessageFuture(child)
	defer c.RemoveMessageFuture(other)
	getty.Phase0NativeMerge(id, &message.MergedWarpMessage{Msgs: []message.MessageTypeAware{nativeRequest(), nativeRequest()}, MsgIds: []int32{child, other}})
	f := getty.Phase0NativeFuture(child)
	getty.Phase0NativeFuture(other)
	func() {
		defer func() {
			if p := recover(); p != nil {
				t.Errorf("SDK RED actual processor panic: %v", p)
			}
		}()
		nativeDispatch(message.MergeResultMessage{Msgs: []message.MessageTypeAware{nativeReply()}}, id)
	}()
	if f.Response != nil && f.Err == nil {
		t.Error("SDK RED malformed batch partially succeeded")
	}
}
func TestJavaV1SDKBaselineLegacyControl(t *testing.T) {
	id := int32(-22200)
	child := int32(-22201)
	c := getty.GetGettyRemotingClient()
	defer c.RemoveMergedMessageFuture(id)
	defer c.RemoveMessageFuture(child)
	getty.Phase0NativeMerge(id, &message.MergedWarpMessage{Msgs: []message.MessageTypeAware{nativeRequest()}, MsgIds: []int32{child}})
	f := getty.Phase0NativeFuture(child)
	nativeDispatch(message.MergeResultMessage{Msgs: []message.MessageTypeAware{nativeReply()}}, id)
	select {
	case <-f.Done:
	default:
		t.Fatal("native legacy control failed")
	}
}
