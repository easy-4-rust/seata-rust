// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import "seata.apache.org/seata-go/v2/pkg/protocol/message"

// Phase0SDKStoreFuture 仅向测试编译单元暴露已有私有状态，不进入候选补丁。
func Phase0SDKStoreFuture(id int32, body interface{}) *message.MessageFuture {
	f := message.NewMessageFuture(message.RpcMessage{ID: id, Body: body})
	GetGettyRemotingClient().gettyRemoting.futures.Store(id, f)
	return f
}

// Phase0SDKStoreMerge 仅供验证原processor逐子请求路径。
func Phase0SDKStoreMerge(id int32, request *message.MergedWarpMessage) {
	GetGettyRemotingClient().gettyRemoting.mergeMsgMap.Store(id, request)
}

// Phase0SDKClear 保证每个测试只清理自己创建的关联记录。
func Phase0SDKClear(outer int32, ids ...int32) {
	c := GetGettyRemotingClient()
	c.RemoveMergedMessageFuture(outer)
	c.RemoveMessageFuture(outer)
	for _, id := range ids {
		c.RemoveMessageFuture(id)
	}
}

// Phase0SDKState 仅观察future与merge缓存计数。
func Phase0SDKState() (int, int) {
	c := GetGettyRemotingClient()
	f, m := 0, 0
	c.gettyRemoting.futures.Range(func(_, _ interface{}) bool { f++; return true })
	c.gettyRemoting.mergeMsgMap.Range(func(_, _ interface{}) bool { m++; return true })
	return f, m
}

// Phase0SDKAwait 使用原SDK等待路径，供真实processor交付后的契约断言。
func Phase0SDKAwait(future *message.MessageFuture, body interface{}) (interface{}, error) {
	c := GetGettyRemotingClient()
	plan, e := javaV1PlanFor(body)
	if e != nil {
		return nil, e
	}
	return c.javaV1Callback(plan, false)(message.RpcMessage{ID: future.ID, Body: body}, future)
}
