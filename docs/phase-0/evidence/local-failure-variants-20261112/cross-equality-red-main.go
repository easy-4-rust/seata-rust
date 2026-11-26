package main

import (
    "bytes"
    "encoding/hex"
    "fmt"
    "os"
    "seata.apache.org/seata-go/v2/pkg/protocol/codec"
    "seata.apache.org/seata-go/v2/pkg/protocol/message"
    seataerrors "seata.apache.org/seata-go/v2/pkg/util/errors"
)

func main() {
    codec.Init()
    manager := codec.GetCodecManager()
    cases := []struct {name string; value message.MessageTypeAware; java string} {
        {"BranchReportResponseFailed", message.BranchReportResponse{AbstractTransactionResponse: message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeFailed, Msg: "report denied"}, TransactionErrorCode: seataerrors.TransactionErrorCodeBranchReportFailed}}, "000e00000d7265706f72742064656e69656407"},
        {"GlobalLockQueryResponseFailed", message.GlobalLockQueryResponse{AbstractTransactionResponse: message.AbstractTransactionResponse{AbstractResultMessage: message.AbstractResultMessage{ResultCode: message.ResultCodeFailed, Msg: "lock check failed"}, TransactionErrorCode: seataerrors.TransactionErrorCodeLockableCheckFailed}, Lockable:false}, "00160000116c6f636b20636865636b206661696c6564080000"},
    }
    mismatch := false
    for _, sample := range cases {
        actual := manager.Encode(codec.CodecTypeSeata, sample.value)
        expected, err := hex.DecodeString(sample.java)
        if err != nil {panic(err)}
        fmt.Printf("%s Go=%x Java=%x equal=%v\n", sample.name, actual, expected, bytes.Equal(actual,expected))
        mismatch = mismatch || !bytes.Equal(actual,expected)
    }
    if mismatch {fmt.Println("FAIL: pinned Go failure payloads do not equal Java V1 authority"); os.Exit(1)}
}
