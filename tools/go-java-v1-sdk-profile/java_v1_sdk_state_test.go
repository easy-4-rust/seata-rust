// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import (
	"errors"
	"sync"
	"testing"
	"time"

	"seata.apache.org/seata-go/v2/pkg/discovery"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"seata.apache.org/seata-go/v2/pkg/remoting/config"
)

func sdkStatusRequest() message.MessageTypeAware {
	return message.GlobalStatusRequest{AbstractGlobalEndRequest: message.AbstractGlobalEndRequest{Xid: "x"}}
}
func sdkStatusReply() message.MessageTypeAware { return message.GlobalStatusResponse{} }

func TestJavaV1SDKSessionChoiceCapturedAndCompressionRejected(t *testing.T) {
	previous, once := sessionManager, onceSessionManager
	sessionManager = nil
	onceSessionManager = &sync.Once{}
	t.Cleanup(func() { sessionManager = previous; onceSessionManager = once })
	cfg := &config.Config{JavaV1Profile: true}
	InitGetty(cfg, &config.SeataConfig{})
	cfg.JavaV1Profile = false
	for i := 0; i < 2; i++ {
		handler, e := sessionManager.sessionPackageHandler()
		if e != nil || handler.(*RpcPackageHandler).PayloadCodec == nil {
			t.Fatal("session/reconnect lost captured choice", e)
		}
	}
	cfg.SessionConfig.CompressEncoding = true
	if sessionManager.gettyConf.SessionConfig.CompressEncoding {
		t.Fatal("external configuration mutation changed captured session settings")
	}
	sessionManager.gettyConf.SessionConfig.CompressEncoding = true
	if e := sessionManager.newSession(nil); e == nil {
		t.Fatal("incompatible compression fell back to native or touched nil connection")
	}
	native := &SessionManager{}
	handler, e := native.sessionPackageHandler()
	if e != nil || handler != rpcPkgHandler {
		t.Fatal("default handler changed", e)
	}
	if codec.GetCodecManager().GetCodec(codec.CodecTypeSeata, message.MessageTypeSeataMerge) != nil {
		t.Fatal("global registry changed")
	}
}

func TestJavaV1SDKRejectsConflictingProfileBeforeManagerInit(t *testing.T) {
	previous, once := sessionManager, onceSessionManager
	sessionManager = nil
	onceSessionManager = &sync.Once{}
	t.Cleanup(func() { sessionManager = previous; onceSessionManager = once })
	InitGetty(&config.Config{JavaV1Profile: true, SessionConfig: config.SessionConfig{CompressEncoding: true}}, &config.SeataConfig{})
	if sessionManager != nil {
		t.Fatal("invalid configuration reached manager/connection initialization")
	}
}

func TestJavaV1SDKReconnectIgnoresStaleOwnerAndRemovedServer(t *testing.T) {
	starts := 0
	manager := newTestSessionManager(func(*discovery.ServiceInstance) closeableClient { starts++; return &fakeServerClient{} })
	manager.javaV1Profile = true
	manager.gettyConf.ReconnectInterval = 1
	instance := &discovery.ServiceInstance{Addr: "127.0.0.1", Port: 28092}
	address := serverAddress(instance)
	manager.refreshServerList([]*discovery.ServiceInstance{instance})
	value, _ := manager.serverClients.Load(address)
	old := value.(*serverClientEntry)
	manager.reconnectJavaV1(address, old)
	value, _ = manager.serverClients.Load(address)
	current := value.(*serverClientEntry)
	if starts != 2 || current == old {
		t.Fatal("EOF did not replace exact owner")
	}
	manager.reconnectJavaV1(address, old)
	if starts != 2 {
		t.Fatal("stale close callback replaced current client")
	}
	manager.refreshServerList(nil)
	manager.reconnectJavaV1(address, current)
	if starts != 2 {
		t.Fatal("removed registry address resurrected")
	}
}

func TestJavaV1SDKReconnectRechecksRemovalAfterDelay(t *testing.T) {
	started, closed := make(chan string, 3), make(chan string, 3)
	manager := newTestSessionManager(func(instance *discovery.ServiceInstance) closeableClient {
		address := serverAddress(instance)
		started <- address
		return &fakeServerClient{address: address, closed: closed}
	})
	manager.javaV1Profile = true
	manager.gettyConf.ReconnectInterval = int(50 * time.Millisecond)
	instance := &discovery.ServiceInstance{Addr: "127.0.0.1", Port: 28092}
	address := serverAddress(instance)
	manager.refreshServerList([]*discovery.ServiceInstance{instance})
	<-started
	value, _ := manager.serverClients.Load(address)
	done := make(chan struct{})
	go func() { manager.reconnectJavaV1(address, value.(*serverClientEntry)); close(done) }()
	select {
	case <-closed:
	case <-time.After(time.Second):
		t.Fatal("old client not closed")
	}
	manager.refreshServerList(nil)
	<-done
	select {
	case <-started:
		t.Fatal("address removed during delay was restarted")
	default:
	}
}

func TestJavaV1SDKReplyPlanSnapshotAndTypedUnknown(t *testing.T) {
	request := message.MergedWarpMessage{Msgs: []message.MessageTypeAware{sdkStatusRequest(), sdkStatusRequest()}}
	plan, e := javaV1PlanFor(&request)
	if e != nil {
		t.Fatal(e)
	}
	request.Msgs[0] = message.GlobalBeginRequest{}
	if e := plan.validate(message.MergeResultMessage{Msgs: []message.MessageTypeAware{sdkStatusReply(), sdkStatusReply()}}); e != nil {
		t.Fatal("caller mutation changed snapshot", e)
	}
	for _, reply := range []interface{}{nil, (*message.MergeResultMessage)(nil), message.GlobalBeginResponse{}, message.MergeResultMessage{}, message.MergeResultMessage{Msgs: []message.MessageTypeAware{sdkStatusReply(), message.GlobalBeginResponse{}}}} {
		var unknown *JavaV1UnknownReplyError
		if e := plan.validate(reply); !errors.As(e, &unknown) {
			t.Fatal("missing typed unknown error", e)
		}
	}
	c := &GettyRemotingClient{gettyRemoting: newGettyRemoting()}
	future := message.NewMessageFuture(message.RpcMessage{ID: 9})
	c.gettyRemoting.futures.Store(int32(9), future)
	future.Complete(message.MergeResultMessage{})
	if _, e := c.javaV1Callback(plan, false)(message.RpcMessage{ID: 9}, future); e == nil {
		t.Fatal("SDK callback accepted inconsistent reply")
	}
	if c.GetMessageFuture(9) != nil {
		t.Fatal("failed reply future leaked")
	}
}

func TestJavaV1SDKRejectsInvalidBatchBeforeSelectingConnection(t *testing.T) {
	c := &GettyRemotingClient{}
	for _, request := range []message.MergedWarpMessage{
		{Msgs: []message.MessageTypeAware{sdkStatusRequest()}, MsgIds: []int32{1, 2}},
		{Msgs: []message.MessageTypeAware{nil}},
		{Msgs: []message.MessageTypeAware{message.UndoLogDeleteRequest{}}},
		{Msgs: make([]message.MessageTypeAware, 1025)},
	} {
		if _, e := c.SendSyncRequest(request); e == nil {
			t.Fatal("invalid request reached connection/id generator")
		}
		if e := c.SendAsyncRequest(request); e == nil {
			t.Fatal("invalid async request reached connection")
		}
	}
}
