// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty_test

import (
	"errors"
	"reflect"
	"runtime"
	"sync"
	"testing"
	"time"

	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"seata.apache.org/seata-go/v2/pkg/remoting/config"
	getty "seata.apache.org/seata-go/v2/pkg/remoting/getty"
	client "seata.apache.org/seata-go/v2/pkg/remoting/processor/client"
)

func dispatchSDK(body interface{}, id int32) {
	config.InitTransportConfig(&config.TransportConfig{Protocol: "seata"})
	client.RegisterProcessor()
	getty.GetGettyClientHandlerInstance().OnMessage(nil, message.RpcMessage{ID: id, Type: 1, Codec: 1, Body: body})
}

func TestJavaV1SDKInconsistentEnvelopeAndUnregisteredReplyType(t *testing.T) {
	for _, c := range []struct {
		name string
		kind message.RequestType
		body interface{}
	}{
		{"unregisteredBranchCommit", 1, message.BranchCommitResponse{}}, {"unregisteredBranchRollback", 1, message.BranchRollbackResponse{}},
		{"requestKindWithReplyBody", 0, statusReply()}, {"onewayKindWithReplyBody", 2, statusReply()},
		{"replyKindWithRequestBody", 1, message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: "x"}}},
	} {
		t.Run(c.name, func(t *testing.T) {
			id := int32(-21300)
			defer getty.Phase0SDKClear(id)
			f := getty.Phase0SDKStoreFuture(id, statusRequest())
			config.InitTransportConfig(&config.TransportConfig{Protocol: "seata"})
			client.RegisterProcessor()
			getty.GetGettyClientHandlerInstance().OnMessage(nil, message.RpcMessage{ID: id, Type: c.kind, Codec: 1, Body: c.body})
			if getty.GetGettyRemotingClient().GetMessageFuture(id) != nil {
				t.Fatal("valid inconsistent reply was dropped before unknown-outcome validation")
			}
			_, e := getty.Phase0SDKAwait(f, statusRequest())
			var unknown *getty.JavaV1UnknownReplyError
			if !errors.As(e, &unknown) {
				t.Fatal("inconsistent envelope/type returned success or generic error", e)
			}
		})
	}
}

func TestJavaV1SDKConcurrentMergeRepliesCannotMixChildren(t *testing.T) {
	outer := int32(-21400)
	defer getty.Phase0SDKClear(outer, -21401, -21402)
	a := getty.Phase0SDKStoreFuture(-21401, statusRequest())
	a.Done = make(chan struct{})
	b := getty.Phase0SDKStoreFuture(-21402, statusRequest())
	getty.Phase0SDKStoreMerge(outer, &message.MergedWarpMessage{Msgs: []message.MessageTypeAware{statusRequest(), statusRequest()}, MsgIds: []int32{-21401, -21402}})
	first := statusReply().(message.GlobalStatusResponse)
	second := first
	second.GlobalStatus = 15
	config.InitTransportConfig(&config.TransportConfig{Protocol: "seata"})
	client.RegisterProcessor()
	send := func(reply message.GlobalStatusResponse) {
		getty.GetGettyClientHandlerInstance().OnMessage(nil, message.RpcMessage{ID: outer, Type: 1, Codec: 1, Body: message.MergeResultMessage{Msgs: []message.MessageTypeAware{reply, reply}}})
	}
	var wait sync.WaitGroup
	wait.Add(2)
	go func() { defer wait.Done(); send(first) }()
	runtime.Gosched()
	time.Sleep(10 * time.Millisecond)
	started := make(chan struct{})
	go func() { defer wait.Done(); close(started); send(second) }()
	<-started
	// 首个处理器阻塞于发布Done；原实现允许第二处理器越过CAS并完成另一个子项。
	time.Sleep(30 * time.Millisecond)
	<-a.Done
	finished := make(chan struct{})
	go func() { wait.Wait(); close(finished) }()
	select {
	case <-finished:
	case <-time.After(time.Second):
		t.Fatal("reply dispatch blocked")
	}
	if !reflect.DeepEqual(a.Response, b.Response) {
		t.Fatal("children came from different duplicate containers")
	}
}
func statusRequest() message.MessageTypeAware {
	return message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: "x"}}
}
func statusReply() message.MessageTypeAware {
	return message.GlobalStatusResponse{AbstractGlobalEndResponse: message.AbstractGlobalEndResponse{AbstractTransactionResponse: message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: 1}}, GlobalStatus: 1}}
}

func TestJavaV1SDKDeliversWholeMergedReplyToOuterFuture(t *testing.T) {
	id := int32(-21000)
	defer getty.Phase0SDKClear(id)
	request := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{statusRequest(), statusRequest()}, MsgIds: []int32{7, -9}}
	f := getty.Phase0SDKStoreFuture(id, request)
	reply := message.MergeResultMessage{Msgs: []message.MessageTypeAware{statusReply(), statusReply()}}
	dispatchSDK(reply, id)
	select {
	case <-f.Done:
		if !reflect.DeepEqual(f.Response, reply) {
			t.Fatal("whole merged body changed")
		}
	default:
		t.Fatal("actual original processor dropped valid whole-batch reply")
	}
	if getty.GetGettyRemotingClient().GetMessageFuture(id) != nil {
		t.Fatal("outer future leaked")
	}
	dispatchSDK(reply, id)
	select {
	case <-f.Done:
		t.Fatal("duplicate completed same future twice")
	default:
	}
}

func TestJavaV1SDKRejectsInconsistentLegacyMergeBeforeCompletion(t *testing.T) {
	cases := []struct {
		name    string
		ids     []int32
		results []message.MessageTypeAware
	}{
		{"shortResults", []int32{-21101, -21102}, []message.MessageTypeAware{statusReply()}},
		{"missingIds", []int32{-21101}, []message.MessageTypeAware{statusReply(), statusReply()}},
		{"extraResults", []int32{-21101, -21102}, []message.MessageTypeAware{statusReply(), statusReply(), statusReply()}},
		{"duplicateIds", []int32{-21101, -21101}, []message.MessageTypeAware{statusReply(), statusReply()}},
		{"wrongResultType", []int32{-21101, -21102}, []message.MessageTypeAware{statusReply(), message.GlobalBeginResponse{}}},
		{"nilResult", []int32{-21101, -21102}, []message.MessageTypeAware{statusReply(), nil}},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			outer := int32(-21100)
			defer getty.Phase0SDKClear(outer, -21101, -21102)
			getty.Phase0SDKStoreMerge(outer, &message.MergedWarpMessage{Msgs: []message.MessageTypeAware{statusRequest(), statusRequest()}, MsgIds: c.ids})
			a := getty.Phase0SDKStoreFuture(-21101, statusRequest())
			b := getty.Phase0SDKStoreFuture(-21102, statusRequest())
			func() {
				defer func() {
					if p := recover(); p != nil {
						t.Errorf("actual processor panicked: %v", p)
					}
				}()
				dispatchSDK(message.MergeResultMessage{Msgs: c.results}, outer)
			}()
			for index, f := range []*message.MessageFuture{a, b} {
				id := int32(-21101 - index)
				known := false
				for _, cached := range c.ids {
					if cached == id {
						known = true
					}
				}
				select {
				case <-f.Done:
					var unknown *getty.JavaV1UnknownReplyError
					if !known || f.Response != nil || !errors.As(f.Err, &unknown) {
						t.Error("inconsistent batch became success or lost typed unknown error")
					}
				default:
					if known {
						t.Error("known child still waits instead of typed unknown outcome")
					}
				}
			}
			if getty.GetGettyRemotingClient().GetMergedMessage(outer) != nil {
				t.Error("inconsistent merge correlation leaked")
			}
		})
	}
}

func TestJavaV1SDKLegacyMergeRetainsIdsAndDropsDuplicates(t *testing.T) {
	outer := int32(-21200)
	defer getty.Phase0SDKClear(outer, -21201, -21202, -21203)
	a := getty.Phase0SDKStoreFuture(-21201, statusRequest())
	b := getty.Phase0SDKStoreFuture(-21202, statusRequest())
	unrelated := getty.Phase0SDKStoreFuture(-21203, statusRequest())
	getty.Phase0SDKStoreMerge(outer, &message.MergedWarpMessage{Msgs: []message.MessageTypeAware{statusRequest(), statusRequest()}, MsgIds: []int32{-21202, -21201}})
	one := statusReply().(message.GlobalStatusResponse)
	one.GlobalStatus = 15
	two := statusReply()
	reply := message.MergeResultMessage{Msgs: []message.MessageTypeAware{one, two}}
	dispatchSDK(reply, outer)
	if !reflect.DeepEqual(b.Response, one) || !reflect.DeepEqual(a.Response, two) {
		t.Fatal("child ID mapping changed")
	}
	for _, f := range []*message.MessageFuture{a, b} {
		select {
		case <-f.Done:
		default:
			t.Fatal("child not completed")
		}
	}
	dispatchSDK(reply, outer)
	for _, f := range []*message.MessageFuture{a, b, unrelated} {
		select {
		case <-f.Done:
			t.Fatal("duplicate/unrelated completion")
		default:
		}
	}
	if getty.GetGettyRemotingClient().GetMergedMessage(outer) != nil {
		t.Fatal("merged correlation leaked")
	}
}
