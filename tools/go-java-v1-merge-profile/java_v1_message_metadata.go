// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package codec

import (
	"fmt"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	"unicode/utf8"
)

// JavaV1VersionInfo 显式区分线缆字段缺省与Java getter默认值。
// 对应Java: AbstractIdentifyResponse#getVersion / AbstractIdentifyResponseCodec#decode。
type JavaV1VersionInfo struct {
	// WireVersion为nil表示零长度字段；不能把它当成Java getVersion()==空字符串。
	WireVersion *string
	// JavaVersion是在调用方提供的Java当前版本上下文中的getter语义。
	JavaVersion string
}

// RegistrationVersion 输入102/104原生值及Java默认版本上下文，返回可选线缆字段与Java语义；不改写原对象或字节。
func (p *JavaV1MessageProfile) RegistrationVersion(in interface{}, javaCurrent string) (*JavaV1VersionInfo, error) {
	if p == nil || p.manager == nil || p.responses == nil {
		return nil, fmt.Errorf("uninitialized V1 message profile")
	}
	value, kind, e := javaV1Value(in)
	if e != nil {
		return nil, e
	}
	var raw string
	switch kind {
	case 102:
		raw = value.(message.RegisterTMResponse).Version
	case 104:
		raw = value.(message.RegisterRMResponse).Version
	default:
		return nil, fmt.Errorf("registration response required")
	}
	if raw != "" {
		if len(raw) > 32767 || !utf8.ValidString(raw) {
			return nil, fmt.Errorf("invalid wire version")
		}
		return &JavaV1VersionInfo{WireVersion: &raw, JavaVersion: raw}, nil
	}
	if javaCurrent == "" || !utf8.ValidString(javaCurrent) {
		return nil, fmt.Errorf("Java current-version context required for absent field")
	}
	return &JavaV1VersionInfo{WireVersion: nil, JavaVersion: javaCurrent}, nil
}

// ReportStatus 输入全局报告请求，返回状态及是否存在；255缺省不同于状态0 Unknown，未知码返回错误。
func (p *JavaV1MessageProfile) ReportStatus(in interface{}) (message.GlobalStatus, bool, error) {
	if p == nil || p.manager == nil || p.responses == nil {
		return 0, false, fmt.Errorf("uninitialized V1 message profile")
	}
	value, kind, e := javaV1Value(in)
	if e != nil {
		return 0, false, e
	}
	if kind != 17 {
		return 0, false, fmt.Errorf("global report request required")
	}
	status := value.(message.GlobalReportRequest).GlobalStatus
	if status == 255 {
		return 0, false, nil
	}
	if status > 20 {
		return 0, false, fmt.Errorf("unknown Java global status")
	}
	return status, true, nil
}
