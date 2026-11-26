// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import "seata.apache.org/seata-go/v2/pkg/protocol/message"

// Phase0NativeFuture 仅暴露原状态用于基线行为测试，不进入候选补丁。
func Phase0NativeFuture(id int32) *message.MessageFuture {
	f := message.NewMessageFuture(message.RpcMessage{ID: id})
	GetGettyRemotingClient().gettyRemoting.futures.Store(id, f)
	return f
}

// Phase0NativeMerge 仅建立基线已存在的合并关联。
func Phase0NativeMerge(id int32, request *message.MergedWarpMessage) {
	GetGettyRemotingClient().gettyRemoting.mergeMsgMap.Store(id, request)
}
