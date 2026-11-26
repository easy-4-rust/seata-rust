// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

import "fmt"

// JavaV1TransportError 保留发送阶段与原始原因；结果未知不表示TC拒绝，更不允许自动重发。
// 对应Java: NettyClient请求等待/连接异常边界；正式Rust错误API遵循ADR-0008另行实现。
type JavaV1TransportError struct {
	ID             int32
	Classification string
	Cause          error
}

// Error 只输出请求ID、阶段与原因，不包含业务消息正文。
func (e *JavaV1TransportError) Error() string {
	return fmt.Sprintf("Java V1 request %d %s: %v", e.ID, e.Classification, e.Cause)
}

// Outcome 返回 LocalFailureBeforeSend 或 UnknownOutcome。
func (e *JavaV1TransportError) Outcome() string { return e.Classification }

// RequestID 返回分配的外层ID；分配前的本地错误为0。
func (e *JavaV1TransportError) RequestID() int32 { return e.ID }

// Unwrap 供errors.Is/As检查超时、连接或写错误的原始原因。
func (e *JavaV1TransportError) Unwrap() error { return e.Cause }
