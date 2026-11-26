// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package getty

// JavaV1UnknownReplyError 表示响应与请求不一致，不能据此认定事务被拒绝或安全重发。
// 对应Java: NettyClient同步请求/合并结果关联边界；生产Rust错误API另行实现。
type JavaV1UnknownReplyError struct{ Reason string }

// Error 返回包含“结果未知”语义的说明。
func (e *JavaV1UnknownReplyError) Error() string {
	return "Java V1 reply inconsistent; transaction outcome unknown: " + e.Reason
}
