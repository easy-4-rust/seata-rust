// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import (
	"context"
	"errors"
	getty "github.com/apache/dubbo-getty"
	uberatomic "go.uber.org/atomic"
	"seata.apache.org/seata-go/v2/pkg/protocol/codec"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"seata.apache.org/seata-go/v2/pkg/remoting/config"
	"seata.apache.org/seata-go/v2/pkg/remoting/processor"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

type inflightTestSession struct {
	getty.Session
	closed  atomic.Bool
	writes  atomic.Int32
	address string
	write   func(message.RpcMessage) (int, int, error)
}

func (s *inflightTestSession) IsClosed() bool                       { return s.closed.Load() }
func (s *inflightTestSession) RemoteAddr() string                   { return s.address }
func (s *inflightTestSession) Stat() string                         { return s.address }
func (s *inflightTestSession) Close()                               { s.closed.Store(true) }
func (s *inflightTestSession) GetAttribute(interface{}) interface{} { return nil }
func (s *inflightTestSession) WritePkg(body interface{}, _ time.Duration) (int, int, error) {
	s.writes.Add(1)
	return s.write(body.(message.RpcMessage))
}

type inflightProcessor struct{ client *GettyRemotingClient }
type inflightInboundProcessor struct{ handle func(message.RpcMessage) }
type inflightBarrierProcessor struct {
	client                      *GettyRemotingClient
	entered, release, processed chan struct{}
}

func (p inflightBarrierProcessor) Process(ctx context.Context, r message.RpcMessage) error {
	if r.Body.(message.GlobalBeginResponse).Xid == "old" {
		close(p.entered)
		<-p.release
		defer close(p.processed)
	}
	return (inflightProcessor{p.client}).Process(ctx, r)
}

func (p inflightInboundProcessor) Process(_ context.Context, rpc message.RpcMessage) error {
	p.handle(rpc)
	return nil
}

func (p inflightProcessor) Process(ctx context.Context, rpc message.RpcMessage) error {
	if aware, ok := interface{}(p.client).(interface {
		HandleJavaV1BoundResponse(context.Context, message.RpcMessage) (bool, error)
	}); ok {
		if handled, e := aware.HandleJavaV1BoundResponse(ctx, rpc); handled {
			return e
		}
	}
	if _, ok := rpc.Body.(message.MergeResultMessage); ok {
		return p.client.HandleJavaV1MergedResponse(rpc)
	}
	p.client.NotifyRpcMessageResponse(rpc)
	return nil
}

func inflightSetup(t *testing.T) (*GettyRemotingClient, *gettyClientHandler) {
	codec.Init()
	oldManager, oldClient, oldHandler, oldTimeout := sessionManager, gettyRemotingClient, clientHandler, rpcRequestTimeout
	sessionManager = newTestSessionManager(nil)
	sessionManager.javaV1Profile = true
	sessionManager.gettyConf = &config.Config{JavaV1Profile: true}
	c := &GettyRemotingClient{idGenerator: &uberatomic.Uint32{}, gettyRemoting: newGettyRemoting()}
	gettyRemotingClient = c
	h := &gettyClientHandler{}
	clientHandler = h
	h.processorMap = make(map[message.MessageType]processor.RemotingProcessor)
	rpcRequestTimeout = 25 * time.Millisecond
	t.Cleanup(func() {
		c.gettyRemoting.futures.Range(func(_, v interface{}) bool {
			v.(*message.MessageFuture).CompleteWithError(nil, errors.New("owned test cleanup"))
			return true
		})
		sessionManager = oldManager
		gettyRemotingClient = oldClient
		clientHandler = oldHandler
		rpcRequestTimeout = oldTimeout
	})
	h.RegisterProcessor(message.MessageType(2), inflightProcessor{c})
	return c, h
}

func inflightRequest(id int32) message.RpcMessage {
	return message.RpcMessage{ID: id, Type: message.RequestTypeRequestSync, Codec: 1, Body: message.GlobalBeginRequest{Timeout: time.Second, TransactionName: "inflight"}}
}
func inflightReply(id int32, xid string) message.RpcMessage {
	return message.RpcMessage{ID: id, Type: message.RequestTypeResponse, Codec: 1, Body: message.GlobalBeginResponse{Xid: xid}}
}
func inflightOutcome(t *testing.T, err error, wanted string, id int32) {
	t.Helper()
	var detail interface {
		Outcome() string
		RequestID() int32
	}
	if !errors.As(err, &detail) || detail.Outcome() != wanted || detail.RequestID() != id {
		t.Errorf("want typed %s request%d, got %T: %v", wanted, id, err, err)
	}
}
func inflightClean(t *testing.T, c *GettyRemotingClient, id int32) {
	t.Helper()
	if c.GetMessageFuture(id) != nil {
		t.Error("owned future leaked")
	}
}

func TestJavaV1InflightClosedSessionIsLocal(t *testing.T) {
	c, _ := inflightSetup(t)
	s := &inflightTestSession{address: "closed"}
	s.closed.Store(true)
	_, err := c.gettyRemoting.SendSync(inflightRequest(71), s, c.syncCallback)
	inflightOutcome(t, err, "LocalFailureBeforeSend", 71)
	if s.writes.Load() != 0 {
		t.Error("closed session written")
	}
	inflightClean(t, c, 71)
}
func TestJavaV1InflightEncodingFailureIsLocal(t *testing.T) {
	c, _ := inflightSetup(t)
	s := &inflightTestSession{address: "encoding", write: func(message.RpcMessage) (int, int, error) { return 0, 0, errors.New("must preflight before WritePkg") }}
	req := inflightRequest(72)
	req.Compressor = 7
	_, err := c.gettyRemoting.SendSync(req, s, c.syncCallback)
	inflightOutcome(t, err, "LocalFailureBeforeSend", 72)
	if s.writes.Load() != 0 {
		t.Error("invalid frame reached WritePkg")
	}
	inflightClean(t, c, 72)
}
func TestJavaV1InflightWriteFailureIsUnknownEvenWhenZero(t *testing.T) {
	c, _ := inflightSetup(t)
	cause := errors.New("write interrupted")
	s := &inflightTestSession{address: "write", write: func(message.RpcMessage) (int, int, error) { return 0, 0, cause }}
	_, err := c.gettyRemoting.SendSync(inflightRequest(73), s, c.syncCallback)
	inflightOutcome(t, err, "UnknownOutcome", 73)
	if !errors.Is(err, cause) || s.writes.Load() != 1 {
		t.Error("lost write cause or resent request")
	}
	inflightClean(t, c, 73)
}
func TestJavaV1InflightTimeoutIsUnknownAndLateReplyDoesNotRevive(t *testing.T) {
	c, _ := inflightSetup(t)
	s := &inflightTestSession{address: "timeout", write: func(message.RpcMessage) (int, int, error) { return 16, 1, nil }}
	_, err := c.gettyRemoting.SendSync(inflightRequest(74), s, c.syncCallback)
	inflightOutcome(t, err, "UnknownOutcome", 74)
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Error("deadline cause missing")
	}
	c.NotifyRpcMessageResponse(inflightReply(74, "late"))
	inflightClean(t, c, 74)
	if s.writes.Load() != 1 {
		t.Error("unknown operation resent")
	}
}
func TestJavaV1InflightCloseOnlyWakesOwnedSession(t *testing.T) {
	c, h := inflightSetup(t)
	rpcRequestTimeout = time.Second
	written := make(chan int32, 2)
	write := func(r message.RpcMessage) (int, int, error) { written <- r.ID; return 16, 1, nil }
	a := &inflightTestSession{address: "a", write: write}
	b := &inflightTestSession{address: "b", write: write}
	sessionManager.registerSession(a)
	sessionManager.registerSession(b)
	doneA, doneB := make(chan error, 1), make(chan error, 1)
	go func() { _, e := c.gettyRemoting.SendSync(inflightRequest(75), a, c.syncCallback); doneA <- e }()
	go func() { _, e := c.gettyRemoting.SendSync(inflightRequest(76), b, c.syncCallback); doneB <- e }()
	for i := 0; i < 2; i++ {
		select {
		case <-written:
		case <-time.After(250 * time.Millisecond):
			t.Fatal("request failed before reaching WritePkg")
		}
	}
	h.OnClose(a)
	select {
	case e := <-doneA:
		inflightOutcome(t, e, "UnknownOutcome", 75)
	case <-time.After(100 * time.Millisecond):
		t.Error("close did not wake owned request")
		c.GetMessageFuture(75).CompleteWithError(nil, errors.New("cleanup"))
		<-doneA
	}
	select {
	case <-doneB:
		t.Error("other session request failed")
	default:
	}
	c.NotifyRpcMessageResponse(inflightReply(76, "known"))
	if e := <-doneB; e != nil {
		t.Error(e)
	}
	inflightClean(t, c, 75)
	inflightClean(t, c, 76)
}
func TestJavaV1InflightErrorDuringWritePreservesCause(t *testing.T) {
	c, h := inflightSetup(t)
	cause := errors.New("connection lost during WritePkg")
	s := &inflightTestSession{address: "race"}
	sessionManager.registerSession(s)
	s.write = func(message.RpcMessage) (int, int, error) { h.OnError(s, cause); return 16, 1, nil }
	_, err := c.gettyRemoting.SendSync(inflightRequest(77), s, c.syncCallback)
	inflightOutcome(t, err, "UnknownOutcome", 77)
	if !errors.Is(err, cause) {
		t.Error("close cause missing")
	}
	inflightClean(t, c, 77)
}
func TestJavaV1InflightWrongSessionCannotCompleteRequest(t *testing.T) {
	c, h := inflightSetup(t)
	a := &inflightTestSession{address: "owner"}
	b := &inflightTestSession{address: "foreign"}
	a.write = func(req message.RpcMessage) (int, int, error) {
		h.OnMessage(b, inflightReply(req.ID, "foreign"))
		h.OnMessage(a, inflightReply(req.ID, "owner"))
		return 16, 1, nil
	}
	got, err := c.gettyRemoting.SendSync(inflightRequest(78), a, c.syncCallback)
	if err != nil || got.(message.GlobalBeginResponse).Xid != "owner" {
		t.Errorf("foreign connection completed request: %v %v", got, err)
	}
	inflightClean(t, c, 78)
}
func TestJavaV1InflightKnownResponseWinsLaterClose(t *testing.T) {
	c, h := inflightSetup(t)
	s := &inflightTestSession{address: "known"}
	sessionManager.registerSession(s)
	s.write = func(req message.RpcMessage) (int, int, error) {
		h.OnMessage(s, inflightReply(req.ID, "known"))
		h.OnClose(s)
		return 16, 1, nil
	}
	got, err := c.gettyRemoting.SendSync(inflightRequest(79), s, c.syncCallback)
	if err != nil || got.(message.GlobalBeginResponse).Xid != "known" {
		t.Error("known result overwritten", got, err)
	}
	inflightClean(t, c, 79)
}
func TestJavaV1InflightDefaultClosedSessionRetainsLegacyError(t *testing.T) {
	c, _ := inflightSetup(t)
	sessionManager.javaV1Profile = false
	s := &inflightTestSession{address: "legacy"}
	s.closed.Store(true)
	_, err := c.gettyRemoting.SendSync(inflightRequest(80), s, c.syncCallback)
	if err == nil || err.Error() != "session is closed" {
		t.Error("default changed", err)
	}
}

func TestJavaV1InflightInboundRMRequestWithSameIDIsNotDropped(t *testing.T) {
	c, h := inflightSetup(t)
	a := &inflightTestSession{address: "tm-owner"}
	b := &inflightTestSession{address: "rm-inbound"}
	handled := 0
	h.RegisterProcessor(message.MessageType(3), inflightInboundProcessor{func(r message.RpcMessage) { handled++ }})
	a.write = func(req message.RpcMessage) (int, int, error) {
		h.OnMessage(b, message.RpcMessage{ID: req.ID, Type: message.RequestTypeRequestSync, Codec: 1, Body: message.BranchCommitRequest{}})
		h.OnMessage(a, inflightReply(req.ID, "tm"))
		return 16, 1, nil
	}
	got, err := c.gettyRemoting.SendSync(inflightRequest(81), a, c.syncCallback)
	if err != nil || got.(message.GlobalBeginResponse).Xid != "tm" || handled != 1 {
		t.Errorf("independent TC request-ID space was dropped: handled=%d value=%v error=%v", handled, got, err)
	}
}
func TestJavaV1InflightForeignReplyDisguisedAsRequestIsIgnored(t *testing.T) {
	c, h := inflightSetup(t)
	a := &inflightTestSession{address: "owner"}
	b := &inflightTestSession{address: "foreign"}
	a.write = func(req message.RpcMessage) (int, int, error) {
		foreign := inflightReply(req.ID, "foreign")
		foreign.Type = message.RequestTypeRequestSync
		h.OnMessage(b, foreign)
		h.OnMessage(a, inflightReply(req.ID, "owner"))
		return 16, 1, nil
	}
	got, err := c.gettyRemoting.SendSync(inflightRequest(82), a, c.syncCallback)
	if err != nil || got.(message.GlobalBeginResponse).Xid != "owner" {
		t.Error("foreign malformed reply failed owner", got, err)
	}
}
func TestJavaV1InflightDuplicateIDPreservesExistingFuture(t *testing.T) {
	c, _ := inflightSetup(t)
	existing := message.NewMessageFuture(inflightRequest(83))
	c.gettyRemoting.futures.Store(int32(83), existing)
	s := &inflightTestSession{address: "collision", write: func(req message.RpcMessage) (int, int, error) {
		c.NotifyRpcMessageResponse(inflightReply(req.ID, "wrong"))
		return 16, 1, nil
	}}
	_, err := c.gettyRemoting.SendSync(inflightRequest(83), s, c.syncCallback)
	inflightOutcome(t, err, "LocalFailureBeforeSend", 83)
	if c.GetMessageFuture(83) != existing || s.writes.Load() != 0 {
		t.Error("existing owner overwritten or duplicate sent")
	}
	c.RemoveMessageFuture(83)
}
func TestJavaV1InflightResponseAndCloseRacePublishesOneCoherentResult(t *testing.T) {
	c, h := inflightSetup(t)
	for i := 0; i < 30; i++ {
		id := int32(100 + i)
		s := &inflightTestSession{address: "race"}
		sessionManager.registerSession(s)
		s.write = func(req message.RpcMessage) (int, int, error) {
			var wg sync.WaitGroup
			wg.Add(2)
			go func() { defer wg.Done(); h.OnMessage(s, inflightReply(req.ID, "known")) }()
			go func() { defer wg.Done(); h.OnClose(s) }()
			wg.Wait()
			return 16, 1, nil
		}
		got, err := c.gettyRemoting.SendSync(inflightRequest(id), s, c.syncCallback)
		if err != nil {
			inflightOutcome(t, err, "UnknownOutcome", id)
			if got != nil {
				t.Error("mixed success and error")
			}
		} else if got.(message.GlobalBeginResponse).Xid != "known" {
			t.Error("known result differs")
		}
		inflightClean(t, c, id)
	}
}
func TestJavaV1InflightBusinessRejectionRemainsAuthoritativeResponse(t *testing.T) {
	c, h := inflightSetup(t)
	s := &inflightTestSession{address: "rejected"}
	s.write = func(req message.RpcMessage) (int, int, error) {
		reply := inflightReply(req.ID, "")
		body := reply.Body.(message.GlobalBeginResponse)
		body.ResultCode = 0
		body.Msg = "explicit TC refusal"
		reply.Body = body
		h.OnMessage(s, reply)
		return 16, 1, nil
	}
	got, err := c.gettyRemoting.SendSync(inflightRequest(84), s, c.syncCallback)
	if err != nil || got.(message.GlobalBeginResponse).ResultCode != 0 || got.(message.GlobalBeginResponse).Msg != "explicit TC refusal" {
		t.Error("definite rejection flattened into transport uncertainty", got, err)
	}
}

func TestJavaV1InflightPausedOldProcessorCannotCompleteReusedID(t *testing.T) {
	c, h := inflightSetup(t)
	entered, release, processed := make(chan struct{}), make(chan struct{}), make(chan struct{})
	var once sync.Once
	defer once.Do(func() { close(release) })
	h.RegisterProcessor(message.MessageType(2), inflightBarrierProcessor{c, entered, release, processed})
	a := &inflightTestSession{address: "old-owner"}
	b := &inflightTestSession{address: "new-owner"}
	a.write = func(req message.RpcMessage) (int, int, error) {
		go h.OnMessage(a, inflightReply(req.ID, "old"))
		return 16, 1, nil
	}
	first := make(chan error, 1)
	go func() { _, err := c.gettyRemoting.SendSync(inflightRequest(85), a, c.syncCallback); first <- err }()
	select {
	case <-entered:
	case <-time.After(time.Second):
		t.Fatal("old processor did not enter")
	}
	select {
	case err := <-first:
		inflightOutcome(t, err, "UnknownOutcome", 85)
	case <-time.After(time.Second):
		t.Fatal("old caller did not time out")
	}
	written := make(chan struct{})
	b.write = func(message.RpcMessage) (int, int, error) { close(written); return 16, 1, nil }
	type result struct {
		value interface{}
		err   error
	}
	second := make(chan result, 1)
	go func() {
		v, e := c.gettyRemoting.SendSync(inflightRequest(85), b, c.syncCallback)
		second <- result{v, e}
	}()
	select {
	case <-written:
	case <-time.After(time.Second):
		t.Fatal("new owner did not write")
	}
	once.Do(func() { close(release) })
	select {
	case <-processed:
	case <-time.After(time.Second):
		t.Fatal("old processor remained blocked")
	}
	h.OnMessage(b, inflightReply(85, "new"))
	got := <-second
	if got.err != nil || got.value.(message.GlobalBeginResponse).Xid != "new" {
		t.Errorf("old checked response completed reused ID: %v %v", got.value, got.err)
	}
	inflightClean(t, c, 85)
}

func TestJavaV1InflightLegacyNilContextKeepsOriginalProcessorPath(t *testing.T) {
	c, _ := inflightSetup(t)
	if aware, ok := interface{}(c).(interface {
		HandleJavaV1BoundResponse(context.Context, message.RpcMessage) (bool, error)
	}); ok {
		if handled, err := aware.HandleJavaV1BoundResponse(nil, inflightReply(87, "legacy")); handled || err != nil {
			t.Error("nil legacy context was intercepted", handled, err)
		}
	}
}

func inflightLegacyBatch(c *GettyRemotingClient) (*message.MessageFuture, *message.MessageFuture) {
	request := &message.MergedWarpMessage{Msgs: []message.MessageTypeAware{message.GlobalBeginRequest{}, message.GlobalBeginRequest{}}, MsgIds: []int32{91, 92}}
	a, b := message.NewMessageFuture(inflightRequest(91)), message.NewMessageFuture(inflightRequest(92))
	c.gettyRemoting.futures.Store(int32(91), a)
	c.gettyRemoting.futures.Store(int32(92), b)
	c.gettyRemoting.mergeMsgMap.Store(int32(90), request)
	return a, b
}
func inflightLegacyReply(short bool) message.RpcMessage {
	children := []message.MessageTypeAware{message.GlobalBeginResponse{Xid: "one"}, message.GlobalBeginResponse{Xid: "two"}}
	if short {
		children = children[:1]
	}
	return message.RpcMessage{ID: 90, Type: message.RequestTypeResponse, Codec: 1, Body: message.MergeResultMessage{Msgs: children}}
}
func inflightCompleted(t *testing.T, f *message.MessageFuture) {
	t.Helper()
	select {
	case <-f.Done:
	case <-time.After(100 * time.Millisecond):
		t.Fatal("legacy child was not completed")
	}
}
func TestJavaV1InflightLegacyBatchUsesCapturedChildFutures(t *testing.T) {
	c, h := inflightSetup(t)
	a, b := inflightLegacyBatch(c)
	h.RegisterProcessor(message.MessageType(60), inflightProcessor{c})
	h.OnMessage(&inflightTestSession{address: "legacy"}, inflightLegacyReply(false))
	inflightCompleted(t, a)
	inflightCompleted(t, b)
	if a.Err != nil || b.Err != nil || a.Response.(message.GlobalBeginResponse).Xid != "one" || b.Response.(message.GlobalBeginResponse).Xid != "two" {
		t.Error("legacy children differ")
	}
	inflightClean(t, c, 91)
	inflightClean(t, c, 92)
	if c.GetMergedMessage(90) != nil {
		t.Error("legacy cache leaked")
	}
}
func TestJavaV1InflightLegacyBadBatchCompletesAllUnknown(t *testing.T) {
	c, h := inflightSetup(t)
	a, b := inflightLegacyBatch(c)
	h.RegisterProcessor(message.MessageType(60), inflightProcessor{c})
	h.OnMessage(&inflightTestSession{address: "legacy"}, inflightLegacyReply(true))
	inflightCompleted(t, a)
	inflightCompleted(t, b)
	var unknown *JavaV1UnknownReplyError
	if !errors.As(a.Err, &unknown) || !errors.As(b.Err, &unknown) || a.Response != nil || b.Response != nil {
		t.Error("legacy batch produced partial success")
	}
	inflightClean(t, c, 91)
	inflightClean(t, c, 92)
}
func TestJavaV1InflightPausedLegacyBatchDoesNotCompleteNewChildren(t *testing.T) {
	c, h := inflightSetup(t)
	a, b := inflightLegacyBatch(c)
	entered, release, processed := make(chan struct{}), make(chan struct{}), make(chan struct{})
	h.RegisterProcessor(message.MessageType(60), inflightInboundProcessor{func(r message.RpcMessage) {}})
	// 此处理器必须转交原上下文，保证等待期间未来ID所有者变化不能重新关联。
	h.processorMap[message.MessageType(60)] = inflightLegacyBarrier{c, entered, release, processed}
	go h.OnMessage(&inflightTestSession{address: "legacy"}, inflightLegacyReply(false))
	<-entered
	fresh := message.NewMessageFuture(inflightRequest(91))
	c.gettyRemoting.futures.Store(int32(91), fresh)
	close(release)
	<-processed
	inflightCompleted(t, a)
	inflightCompleted(t, b)
	select {
	case <-fresh.Done:
		t.Error("old batch completed new child owner")
	default:
	}
	if c.GetMessageFuture(91) != fresh {
		t.Error("old batch deleted new child owner")
	}
	c.RemoveMessageFuture(91)
}

type inflightLegacyBarrier struct {
	client                      *GettyRemotingClient
	entered, release, processed chan struct{}
}

func (p inflightLegacyBarrier) Process(ctx context.Context, r message.RpcMessage) error {
	close(p.entered)
	<-p.release
	defer close(p.processed)
	return (inflightProcessor{p.client}).Process(ctx, r)
}
