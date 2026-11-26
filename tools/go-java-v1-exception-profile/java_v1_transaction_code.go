// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package errors

import "fmt"

// Java-only domain meanings deliberately do not renumber legacy Go errors.
// They require the explicit Java V1 response profile and must not be byte-cast.
const (
	JavaV1NotRaftLeader       TransactionErrorCode = 256
	JavaV1TransactionTimeout  TransactionErrorCode = 257
	JavaV1CommitHeuristic     TransactionErrorCode = 258
	JavaV1Broken              TransactionErrorCode = 259
	JavaV1BranchPrepareFailed TransactionErrorCode = 260
)

// 对应Java: TransactionExceptionCode；索引是固定Java线缆码，不是Go枚举序号。
var javaV1TransactionCodes = [...]TransactionErrorCode{
	0: TransactionErrorCodeUnknown, 1: TransactionErrorCodeBeginFailed,
	2: TransactionErrorCodeLockKeyConflict, 3: TransactionErrorCodeIO,
	4: TransactionErrorCodeBranchRollbackFailedRetriable, 5: TransactionErrorCodeBranchRollbackFailedUnretriable,
	6: TransactionErrorCodeBranchRegisterFailed, 7: TransactionErrorCodeBranchReportFailed,
	8: TransactionErrorCodeLockableCheckFailed, 9: TransactionErrorCodeBranchTransactionNotExist,
	10: TransactionErrorCodeGlobalTransactionNotExist, 11: TransactionErrorCodeGlobalTransactionNotActive,
	12: TransactionErrorCodeGlobalTransactionStatusInvalid, 13: TransactionErrorCodeFailedToSendBranchCommitRequest,
	14: TransactionErrorCodeFailedToSendBranchRollbackRequest, 15: TransactionErrorCodeFailedToAddBranch,
	16: TransactionErrorCodeFailedLockGlobalTranscation, 17: TransactionErrorCodeFailedWriteSession,
	18: FailedStore, 19: JavaV1NotRaftLeader, 20: LockKeyConflictFailFast,
	21: JavaV1TransactionTimeout, 22: JavaV1CommitHeuristic, 23: JavaV1Broken, 24: JavaV1BranchPrepareFailed,
}

// ToJavaV1TransactionCode 将明确支持的域语义转成Java线缆码；未知/Go专有码返回错误。
func ToJavaV1TransactionCode(code TransactionErrorCode) (byte, error) {
	for wire, domain := range javaV1TransactionCodes {
		if code == domain {
			return byte(wire), nil
		}
	}
	return 0, fmt.Errorf("Go transaction error %d has no Java V1 meaning", code)
}

// TransactionCodeFromJavaV1 返回对应Go域语义；不会把Java19解释成Go的fail-fast。
func TransactionCodeFromJavaV1(wire byte) (TransactionErrorCode, error) {
	if int(wire) >= len(javaV1TransactionCodes) {
		return 0, fmt.Errorf("unknown Java V1 transaction error %d", wire)
	}
	return javaV1TransactionCodes[wire], nil
}
