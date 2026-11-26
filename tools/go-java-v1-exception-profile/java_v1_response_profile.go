// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package codec

import (
	"encoding/binary"
	"fmt"
	"seata.apache.org/seata-go/v2/pkg/protocol/message"
	e "seata.apache.org/seata-go/v2/pkg/util/errors"
	"unicode/utf8"
)

// JavaV1ResponseProfile 是显式选择的事务响应适配器，对应Java: SeataSerializerV1/TransactionExceptionCode。
// 不注册/替换默认codec；要求依赖的type14/22长度修复已应用。
type JavaV1ResponseProfile struct{ manager *CodecManager }

// NewJavaV1ResponseProfile 使用已初始化、稳定的原生registry创建适配入口；nil返回错误。
func NewJavaV1ResponseProfile(manager *CodecManager) (*JavaV1ResponseProfile, error) {
	if manager == nil {
		return nil, fmt.Errorf("nil Java V1 codec manager")
	}
	return &JavaV1ResponseProfile{manager: manager}, nil
}

// Encode 校验并复制一个受支持的事务响应，显式转换异常码；返回新payload或错误，不修改调用方对象。
func (p *JavaV1ResponseProfile) Encode(in interface{}) ([]byte, error) {
	if p == nil || p.manager == nil {
		return nil, fmt.Errorf("uninitialized Java V1 profile")
	}
	value, err := translateJavaV1Response(in, true)
	if err != nil {
		return nil, err
	}
	typed := value.(message.MessageTypeAware)
	if p.manager.GetCodec(CodecTypeSeata, typed.GetTypeCode()) == nil {
		return nil, fmt.Errorf("missing native response codec")
	}
	wire := p.manager.Encode(CodecTypeSeata, value)
	if _, err = validateJavaV1Response(wire); err != nil {
		return nil, err
	}
	return wire, nil
}

// Decode 先严格校验完整payload，再调用原生codec并还原域语义；未知/畸形输入返回错误。
func (p *JavaV1ResponseProfile) Decode(wire []byte) (interface{}, error) {
	if p == nil || p.manager == nil {
		return nil, fmt.Errorf("uninitialized Java V1 profile")
	}
	kind, err := validateJavaV1Response(wire)
	if err != nil {
		return nil, err
	}
	if p.manager.GetCodec(CodecTypeSeata, message.MessageType(kind)) == nil {
		return nil, fmt.Errorf("missing native response codec")
	}
	return translateJavaV1Response(p.manager.Decode(CodecTypeSeata, wire), false)
}

func javaV1String(value string) error {
	if !utf8.ValidString(value) || len(value) > 32767 {
		return fmt.Errorf("invalid/oversized Java V1 UTF8 field")
	}
	return nil
}

func translateJavaV1Response(in interface{}, encode bool) (interface{}, error) {
	// 原生codec要求值对象；显式支持对应非nil指针，再复制其值。
	switch v := in.(type) {
	case *message.GlobalBeginResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.BranchCommitResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.BranchRollbackResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.GlobalCommitResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.GlobalRollbackResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.BranchRegisterResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.BranchReportResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.GlobalStatusResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.GlobalReportResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	case *message.GlobalLockQueryResponse:
		if v == nil {
			return nil, fmt.Errorf("nil response")
		}
		in = *v
	}
	base := func(v *message.AbstractTransactionResponse) error {
		if v.ResultCode != message.ResultCodeFailed && v.ResultCode != message.ResultCodeSuccess {
			return fmt.Errorf("unknown result code")
		}
		if err := javaV1String(v.Msg); err != nil {
			return err
		}
		if v.ResultCode == message.ResultCodeSuccess && v.Msg != "" {
			return fmt.Errorf("success text cannot be preserved on Java wire")
		}
		if encode {
			wire, err := e.ToJavaV1TransactionCode(v.TransactionErrorCode)
			if err != nil {
				return err
			}
			v.TransactionErrorCode = e.TransactionErrorCode(wire)
		} else {
			if v.TransactionErrorCode < 0 || v.TransactionErrorCode > 255 {
				return fmt.Errorf("invalid decoded error code")
			}
			domain, err := e.TransactionCodeFromJavaV1(byte(v.TransactionErrorCode))
			if err != nil {
				return err
			}
			v.TransactionErrorCode = domain
		}
		return nil
	}
	branch := func(v *message.AbstractBranchEndResponse) error {
		if err := base(&v.AbstractTransactionResponse); err != nil {
			return err
		}
		if err := javaV1String(v.Xid); err != nil {
			return err
		}
		if v.BranchStatus < 0 || v.BranchStatus > 15 {
			return fmt.Errorf("unknown branch status")
		}
		return nil
	}
	global := func(v *message.AbstractGlobalEndResponse) error {
		if err := base(&v.AbstractTransactionResponse); err != nil {
			return err
		}
		if v.GlobalStatus > 20 {
			return fmt.Errorf("unknown global status")
		}
		return nil
	}
	switch v := in.(type) {
	case message.GlobalBeginResponse:
		if err := base(&v.AbstractTransactionResponse); err != nil {
			return nil, err
		}
		if err := javaV1String(v.Xid); err != nil {
			return nil, err
		}
		if len(v.ExtraData) > 32767 || !utf8.Valid(v.ExtraData) {
			return nil, fmt.Errorf("invalid/oversized begin extra data")
		}
		return v, nil
	case message.BranchCommitResponse:
		if err := branch(&v.AbstractBranchEndResponse); err != nil {
			return nil, err
		}
		return v, nil
	case message.BranchRollbackResponse:
		if err := branch(&v.AbstractBranchEndResponse); err != nil {
			return nil, err
		}
		return v, nil
	case message.GlobalCommitResponse:
		if err := global(&v.AbstractGlobalEndResponse); err != nil {
			return nil, err
		}
		return v, nil
	case message.GlobalRollbackResponse:
		if err := global(&v.AbstractGlobalEndResponse); err != nil {
			return nil, err
		}
		return v, nil
	case message.BranchRegisterResponse:
		if err := base(&v.AbstractTransactionResponse); err != nil {
			return nil, err
		}
		return v, nil
	case message.BranchReportResponse:
		if err := base(&v.AbstractTransactionResponse); err != nil {
			return nil, err
		}
		return v, nil
	case message.GlobalStatusResponse:
		if err := global(&v.AbstractGlobalEndResponse); err != nil {
			return nil, err
		}
		return v, nil
	case message.GlobalReportResponse:
		if err := global(&v.AbstractGlobalEndResponse); err != nil {
			return nil, err
		}
		return v, nil
	case message.GlobalLockQueryResponse:
		if err := base(&v.AbstractTransactionResponse); err != nil {
			return nil, err
		}
		return v, nil
	default:
		return nil, fmt.Errorf("Java V1 profile requires a supported transaction response")
	}
}

func validateJavaV1Response(wire []byte) (int, error) {
	if len(wire) < 4 || len(wire) > 1024*1024 {
		return 0, fmt.Errorf("invalid response size")
	}
	kind := int(binary.BigEndian.Uint16(wire[:2]))
	at := 2
	take := func(n int) ([]byte, error) {
		if n < 0 || n > len(wire)-at {
			return nil, fmt.Errorf("truncated response")
		}
		value := wire[at : at+n]
		at += n
		return value, nil
	}
	text := func() error {
		length, err := take(2)
		if err != nil {
			return err
		}
		n := int(int16(binary.BigEndian.Uint16(length)))
		data, err := take(n)
		if err != nil {
			return err
		}
		if !utf8.Valid(data) {
			return fmt.Errorf("invalid UTF8 response field")
		}
		return nil
	}
	result, _ := take(1)
	if result[0] > 1 {
		return 0, fmt.Errorf("unknown result code")
	}
	if result[0] == 0 {
		if err := text(); err != nil {
			return 0, err
		}
	}
	errorCode, err := take(1)
	if err != nil {
		return 0, err
	}
	if _, err = e.TransactionCodeFromJavaV1(errorCode[0]); err != nil {
		return 0, err
	}
	switch kind {
	case 2:
		if err := text(); err != nil {
			return 0, err
		}
		if err := text(); err != nil {
			return 0, err
		}
	case 4, 6:
		if err := text(); err != nil {
			return 0, err
		}
		if _, err := take(8); err != nil {
			return 0, err
		}
		status, err := take(1)
		if err != nil {
			return 0, err
		}
		if status[0] > 15 {
			return 0, fmt.Errorf("unknown branch status")
		}
	case 8, 10, 16, 18:
		status, err := take(1)
		if err != nil {
			return 0, err
		}
		if status[0] > 20 {
			return 0, fmt.Errorf("unknown global status")
		}
	case 12:
		if _, err := take(8); err != nil {
			return 0, err
		}
	case 14:
	case 22:
		value, err := take(2)
		if err != nil {
			return 0, err
		}
		if binary.BigEndian.Uint16(value) > 1 {
			return 0, fmt.Errorf("invalid lockable")
		}
	default:
		return 0, fmt.Errorf("unsupported response type %d", kind)
	}
	if at != len(wire) {
		return 0, fmt.Errorf("trailing response bytes")
	}
	return kind, nil
}
