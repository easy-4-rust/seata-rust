// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import (
	"context"
	"errors"
	getty "github.com/apache/dubbo-getty"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"seata.apache.org/seata-go/v2/pkg/remoting/rpc"
	"sync"
	"time"
)

var errJavaV1ConnectionClosed = errors.New("connection closed")

// javaV1Pending 绑定实际会话与原future；mutex只保护开始写入的阶段，绝不持锁调用网络接口。
type javaV1Pending struct {
	mutex     sync.Mutex
	session   getty.Session
	future    *message.MessageFuture
	attempted bool
}

func javaV1TransportEnabled() bool { return sessionManager != nil && sessionManager.javaV1Profile }
func javaV1LocalError(id int32, cause error) error {
	return &JavaV1TransportError{ID: id, Classification: "LocalFailureBeforeSend", Cause: cause}
}
func (p *javaV1Pending) fail(cause error) {
	p.mutex.Lock()
	defer p.mutex.Unlock()
	kind := "LocalFailureBeforeSend"
	if p.attempted {
		kind = "UnknownOutcome"
	}
	p.future.CompleteWithError(nil, &JavaV1TransportError{ID: p.future.ID, Classification: kind, Cause: cause})
}

// sendJavaV1Sync 预检、登记、最多写入一次；完成后只删除本次所有的记录。
func (g *GettyRemoting) sendJavaV1Sync(msg message.RpcMessage, s getty.Session, callback callbackMethod) (interface{}, error) {
	if sessionManager.gettyConf == nil {
		return nil, javaV1LocalError(msg.ID, errors.New("missing captured session configuration"))
	}
	handler, err := sessionManager.sessionPackageHandler()
	if err != nil {
		return nil, javaV1LocalError(msg.ID, err)
	}
	if _, err = handler.Write(s, msg); err != nil {
		return nil, javaV1LocalError(msg.ID, err)
	}
	if s == nil {
		s = sessionManager.selectSession(msg.Body)
	}
	if s == nil || s.IsClosed() {
		return nil, javaV1LocalError(msg.ID, errJavaV1ConnectionClosed)
	}
	future := message.NewMessageFuture(msg)
	entry := &javaV1Pending{session: s, future: future}
	g.javaV1StateMutex.Lock()
	if _, loaded := g.javaV1Pending.LoadOrStore(msg.ID, entry); loaded {
		g.javaV1StateMutex.Unlock()
		return nil, javaV1LocalError(msg.ID, errors.New("request ID already pending"))
	}
	if _, loaded := g.futures.LoadOrStore(msg.ID, future); loaded {
		g.javaV1Pending.CompareAndDelete(msg.ID, entry)
		g.javaV1StateMutex.Unlock()
		return nil, javaV1LocalError(msg.ID, errors.New("request ID already in use"))
	}
	g.javaV1StateMutex.Unlock()
	defer func() {
		g.javaV1StateMutex.Lock()
		defer g.javaV1StateMutex.Unlock()
		g.futures.CompareAndDelete(msg.ID, future)
		g.javaV1Pending.CompareAndDelete(msg.ID, entry)
	}()
	entry.mutex.Lock()
	select {
	case <-future.Done:
		entry.mutex.Unlock()
		return future.Response, future.Err
	default:
	}
	if s.IsClosed() {
		entry.mutex.Unlock()
		return nil, javaV1LocalError(msg.ID, errJavaV1ConnectionClosed)
	}
	entry.attempted = true
	entry.mutex.Unlock()
	rpc.BeginCount(s.RemoteAddr())
	defer rpc.EndCount(s.RemoteAddr())
	// WritePkg返回值不能证明TC未接收；写入期间关闭和返回写错误均保守发布UnknownOutcome。
	_, _, err = s.WritePkg(msg, 0)
	if err != nil {
		entry.fail(err)
	}
	timer := time.NewTimer(rpcRequestTimeout)
	defer timer.Stop()
	select {
	case <-future.Done:
	case <-timer.C:
		entry.fail(context.DeadlineExceeded)
		<-future.Done
	}
	if future.Err != nil {
		return nil, future.Err
	}
	if callback != nil {
		// 原future使用容量1的单消费通知，前面的等待已取走令牌；交还给原SDK callback完成契约校验。
		future.Done <- struct{}{}
		return callback(msg, future)
	}
	return future.Response, nil
}

// failJavaV1Session 只完成实际属于该会话的等待；已发布响应通过future的CAS保持不变。
func (g *GettyRemoting) failJavaV1Session(s getty.Session, cause error) {
	g.javaV1Pending.Range(func(_, value interface{}) bool {
		entry := value.(*javaV1Pending)
		if entry.session == s {
			entry.fail(cause)
		}
		return true
	})
}
