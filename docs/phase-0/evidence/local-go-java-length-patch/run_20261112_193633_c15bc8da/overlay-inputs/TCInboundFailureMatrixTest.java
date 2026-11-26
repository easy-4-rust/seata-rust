/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements. See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
package org.apache.seata.server;

import org.apache.seata.common.exception.StoreException;
import org.apache.seata.core.exception.TransactionException;
import org.apache.seata.core.exception.TransactionExceptionCode;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.core.protocol.ResultCode;
import org.apache.seata.core.protocol.transaction.AbstractGlobalEndResponse;
import org.apache.seata.core.protocol.transaction.AbstractTransactionResponse;
import org.apache.seata.core.protocol.transaction.BranchRegisterRequest;
import org.apache.seata.core.protocol.transaction.BranchRegisterResponse;
import org.apache.seata.core.protocol.transaction.BranchReportRequest;
import org.apache.seata.core.protocol.transaction.BranchReportResponse;
import org.apache.seata.core.protocol.transaction.GlobalBeginRequest;
import org.apache.seata.core.protocol.transaction.GlobalBeginResponse;
import org.apache.seata.core.protocol.transaction.GlobalCommitRequest;
import org.apache.seata.core.protocol.transaction.GlobalCommitResponse;
import org.apache.seata.core.protocol.transaction.GlobalLockQueryRequest;
import org.apache.seata.core.protocol.transaction.GlobalLockQueryResponse;
import org.apache.seata.core.protocol.transaction.GlobalReportRequest;
import org.apache.seata.core.protocol.transaction.GlobalReportResponse;
import org.apache.seata.core.protocol.transaction.GlobalRollbackRequest;
import org.apache.seata.core.protocol.transaction.GlobalRollbackResponse;
import org.apache.seata.core.protocol.transaction.GlobalStatusRequest;
import org.apache.seata.core.protocol.transaction.GlobalStatusResponse;
import org.apache.seata.core.rpc.RpcContext;
import org.apache.seata.serializer.seata.HandlerWireRecord;
import org.apache.seata.serializer.seata.SeataSerializer;
import org.apache.seata.server.session.GlobalSession;
import org.apache.seata.server.session.SessionHolder;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;
import org.mockito.MockedStatic;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.mockStatic;
import static org.mockito.Mockito.when;

/** 真实 Java 入站模板的异常映射和回填矩阵；只替换 do 回调及会话依赖，不替换 handle。 */
class TCInboundFailureMatrixTest {
    private static final String XID = "127.0.0.1:8091:1";
    private static final long BRANCH_ID = 0x0102030405060708L;
    private enum Operation { BEGIN, COMMIT, ROLLBACK, STATUS, REPORT, REGISTER, BRANCH_REPORT, LOCK }
    private enum Failure { NONE, TRANSACTION, RUNTIME, STORE }
    private enum Lookup { ABSENT, THROWS, GETTER_THROWS, MUTATED_THEN_THROWS }

    @TestFactory
    List<DynamicTest> allEightHandlersMapSuccessAndThreeFailureKinds() {
        List<DynamicTest> tests = new ArrayList<>();
        for (Operation op : Operation.values()) {
            for (Failure failure : Failure.values()) {
                tests.add(DynamicTest.dynamicTest(op + "/" + failure, () -> {
                    InjectedHandler handler = configured(op, failure);
                    try (MockedStatic<SessionHolder> sessions = mockStatic(SessionHolder.class)) {
                        GlobalSession session = mock(GlobalSession.class);
                        when(session.getStatus()).thenReturn(GlobalStatus.RollbackRetrying);
                        sessions.when(() -> SessionHolder.findGlobalSession(XID, false)).thenReturn(session);
                        AbstractTransactionResponse response = call(op, handler);
                        GlobalStatus status = failure == Failure.NONE ? successStatus(op)
                                : fallback(op) ? GlobalStatus.RollbackRetrying : GlobalStatus.Rollbacking;
                        assertOutcome("matrix/" + op + "/" + failure, op, failure, response, status);
                        assertEquals(1, handler.calls, "exactly one core callback, no implicit replay");
                        if (fallback(op) && failure != Failure.NONE) {
                            sessions.verify(() -> SessionHolder.findGlobalSession(XID, false));
                        } else { sessions.verifyNoInteractions(); }
                    }
                }));
            }
        }
        return tests;
    }

    @TestFactory
    List<DynamicTest> statusFallbackHandlesAbsentLookupAndGetterFailuresWithoutLosingOriginalError() {
        List<DynamicTest> tests = new ArrayList<>();
        for (Operation op : new Operation[] {Operation.COMMIT, Operation.ROLLBACK, Operation.STATUS}) {
            for (Failure failure : new Failure[] {Failure.TRANSACTION, Failure.RUNTIME, Failure.STORE}) {
                for (Lookup lookup : Lookup.values()) {
                    tests.add(DynamicTest.dynamicTest(op + "/" + failure + "/" + lookup, () -> {
                        InjectedHandler handler = configured(op, failure);
                        if (lookup == Lookup.MUTATED_THEN_THROWS) { handler.statusBeforeFailure = GlobalStatus.StopCommitOrCommitRetry; }
                        try (MockedStatic<SessionHolder> sessions = mockStatic(SessionHolder.class)) {
                            if (lookup == Lookup.ABSENT) {
                                sessions.when(() -> SessionHolder.findGlobalSession(XID, false)).thenReturn(null);
                            } else if (lookup == Lookup.GETTER_THROWS) {
                                GlobalSession session = mock(GlobalSession.class);
                                when(session.getStatus()).thenThrow(new IllegalStateException("status getter failed"));
                                sessions.when(() -> SessionHolder.findGlobalSession(XID, false)).thenReturn(session);
                            } else {
                                sessions.when(() -> SessionHolder.findGlobalSession(XID, false)).thenThrow(new StoreException("fallback store failed"));
                            }
                            AbstractTransactionResponse response = call(op, handler);
                            GlobalStatus expected = lookup == Lookup.ABSENT ? GlobalStatus.Finished
                                    : lookup == Lookup.MUTATED_THEN_THROWS ? GlobalStatus.StopCommitOrCommitRetry : initialStatus(op);
                            assertOutcome("fallback/" + op + "/" + failure + "/" + lookup, op, failure, response, expected);
                            assertEquals(1, handler.calls);
                            sessions.verify(() -> SessionHolder.findGlobalSession(XID, false));
                        }
                    }));
                }
            }
        }
        return tests;
    }

    @TestFactory
    List<DynamicTest> branchRegisterPreservesBothLockConflictCodes() {
        List<DynamicTest> tests = new ArrayList<>();
        for (TransactionExceptionCode code : new TransactionExceptionCode[] {TransactionExceptionCode.LockKeyConflict, TransactionExceptionCode.LockKeyConflictFailFast}) {
            tests.add(DynamicTest.dynamicTest(code.name(), () -> {
                InjectedHandler handler = new InjectedHandler();
                handler.failure = new TransactionException(code, "lock denied");
                try (MockedStatic<SessionHolder> sessions = mockStatic(SessionHolder.class)) {
                    BranchRegisterResponse response = (BranchRegisterResponse) call(Operation.REGISTER, handler);
                    assertEquals(ResultCode.Failed, response.getResultCode());
                    assertEquals(code, response.getTransactionExceptionCode());
                    assertEquals("TransactionException[lock denied]", response.getMsg());
                    assertEquals(0L, response.getBranchId());
                    assertWire("conflict/" + code.name(), Operation.REGISTER, false, code, response.getMsg(), null, response, false);
                    sessions.verifyNoInteractions();
                }
            }));
        }
        return tests;
    }

    @Test
    void successfulLockableFalseRemainsDistinctFromFailure() throws Exception {
        InjectedHandler handler = new InjectedHandler();
        handler.lockable = false;
        GlobalLockQueryResponse response = (GlobalLockQueryResponse) call(Operation.LOCK, handler);
        assertEquals(ResultCode.Success, response.getResultCode());
        assertEquals(TransactionExceptionCode.Unknown, response.getTransactionExceptionCode());
        assertNull(response.getMsg());
        assertFalse(response.isLockable());
        assertWire("lock/success_false", Operation.LOCK, true, TransactionExceptionCode.Unknown, null, null, response, false);
    }

    @Test
    void nullReportStatusFailureIsNotSilentlyConvertedToUnknownOrSuccess() throws Exception {
        InjectedHandler handler = configured(Operation.REPORT, Failure.RUNTIME);
        GlobalReportRequest request = new GlobalReportRequest();
        request.setXid(XID);
        try (MockedStatic<SessionHolder> sessions = mockStatic(SessionHolder.class)) {
            GlobalReportResponse response = handler.handle(request, new RpcContext());
            assertEquals(ResultCode.Failed, response.getResultCode());
            assertEquals("RuntimeException[core failed]", response.getMsg());
            assertNull(response.getGlobalStatus());
            // 固定 Java 的响应编码器不接受 null；这是开放上游边界，不能伪报线缆互操作成功。
            assertThrows(NullPointerException.class, () -> new SeataSerializer((byte) 1).serialize(response));
            HandlerWireRecord.unencodable("report/null_failure", "JAVA-V1-REPORT-NULL-001");
            sessions.verifyNoInteractions();
        }
    }

    private static InjectedHandler configured(Operation op, Failure kind) {
        InjectedHandler handler = new InjectedHandler();
        if (kind == Failure.TRANSACTION) { handler.failure = new TransactionException(txCode(op), "core failed"); }
        if (kind == Failure.RUNTIME) { handler.failure = new IllegalStateException("core failed"); }
        if (kind == Failure.STORE) { handler.failure = new StoreException("store failed"); }
        return handler;
    }

    private static boolean fallback(Operation op) { return op == Operation.COMMIT || op == Operation.ROLLBACK || op == Operation.STATUS; }
    private static GlobalStatus initialStatus(Operation op) {
        return op == Operation.COMMIT ? GlobalStatus.Committing : op == Operation.ROLLBACK ? GlobalStatus.Rollbacking : GlobalStatus.UnKnown;
    }
    private static GlobalStatus successStatus(Operation op) {
        return op == Operation.COMMIT ? GlobalStatus.Committed : op == Operation.ROLLBACK ? GlobalStatus.Rollbacked
                : op == Operation.STATUS ? GlobalStatus.Begin : GlobalStatus.Rollbacking;
    }
    private static TransactionExceptionCode txCode(Operation op) {
        switch (op) {
            case BEGIN: return TransactionExceptionCode.BeginFailed;
            case REGISTER: return TransactionExceptionCode.BranchRegisterFailed;
            case BRANCH_REPORT: return TransactionExceptionCode.BranchReportFailed;
            case LOCK: return TransactionExceptionCode.LockableCheckFailed;
            default: return TransactionExceptionCode.IO;
        }
    }
    private static String storeMessage(Operation op) {
        switch (op) {
            case BEGIN: return "begin global request failed. xid=null, msg=store failed";
            case COMMIT: return "global commit request failed. xid=" + XID + ", msg=store failed";
            case ROLLBACK: return "global rollback request failed. xid=" + XID + ", msg=store failed";
            case STATUS: return "global status request failed. xid=" + XID + ", msg=store failed";
            case REGISTER: return "branch register request failed. xid=" + XID + ", msg=store failed";
            case BRANCH_REPORT: return "branch report request failed. xid=" + XID + ", branchId=" + BRANCH_ID + ", msg=store failed";
            case LOCK: return "global lock query request failed. xid=" + XID + ", msg=store failed";
            default: throw new IllegalArgumentException("GlobalReport does not wrap StoreException");
        }
    }

    private static void assertOutcome(String caseId, Operation op, Failure kind, AbstractTransactionResponse response, GlobalStatus status) throws Exception {
        boolean success = kind == Failure.NONE;
        TransactionExceptionCode code = kind == Failure.TRANSACTION ? txCode(op)
                : kind == Failure.STORE && op != Operation.REPORT ? TransactionExceptionCode.FailedStore : TransactionExceptionCode.Unknown;
        String msg = success ? null : kind == Failure.TRANSACTION ? "TransactionException[core failed]"
                : kind == Failure.STORE && op != Operation.REPORT ? "TransactionException[" + storeMessage(op) + "]"
                : "RuntimeException[" + (kind == Failure.STORE ? "store failed" : "core failed") + "]";
        assertEquals(success ? ResultCode.Success : ResultCode.Failed, response.getResultCode());
        assertEquals(code, response.getTransactionExceptionCode());
        assertEquals(msg, response.getMsg());
        if (response instanceof AbstractGlobalEndResponse) { assertEquals(status, ((AbstractGlobalEndResponse) response).getGlobalStatus()); }
        if (response instanceof GlobalBeginResponse) { assertEquals(success ? XID : null, ((GlobalBeginResponse) response).getXid()); }
        if (response instanceof BranchRegisterResponse) { assertEquals(success ? BRANCH_ID : 0L, ((BranchRegisterResponse) response).getBranchId()); }
        if (response instanceof GlobalLockQueryResponse) { assertEquals(success, ((GlobalLockQueryResponse) response).isLockable()); }
        assertWire(caseId, op, success, code, msg, status, response, op == Operation.LOCK && success);
    }

    private static void string16(DataOutputStream out, String value) throws Exception {
        byte[] bytes = Objects.isNull(value) ? new byte[0] : value.getBytes(StandardCharsets.UTF_8);
        out.writeShort(bytes.length); out.write(bytes);
    }
    private static void assertWire(String caseId, Operation op, boolean success, TransactionExceptionCode code, String msg, GlobalStatus status, AbstractTransactionResponse response, boolean lockable) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        DataOutputStream out = new DataOutputStream(bytes);
        int type = op == Operation.BEGIN ? 2 : op == Operation.COMMIT ? 8 : op == Operation.ROLLBACK ? 10
                : op == Operation.STATUS ? 16 : op == Operation.REPORT ? 18 : op == Operation.REGISTER ? 12 : op == Operation.BRANCH_REPORT ? 14 : 22;
        out.writeShort(type); out.writeByte(success ? 1 : 0);
        if (!success) { string16(out, msg); }
        out.writeByte(code.ordinal());
        if (op == Operation.BEGIN) { string16(out, success ? XID : null); string16(out, null); }
        if (fallback(op) || op == Operation.REPORT) { out.writeByte(status.getCode()); }
        if (op == Operation.REGISTER) { out.writeLong(success ? BRANCH_ID : 0L); }
        if (op == Operation.LOCK) { out.writeShort(lockable ? 1 : 0); }
        byte[] actual = new SeataSerializer((byte) 1).serialize(response);
        assertArrayEquals(bytes.toByteArray(), actual, "independent Java V1 layout");
        HandlerWireRecord.emit(caseId, op.name(), actual, success ? 1 : 0, code.ordinal(), code.name(), msg,
                fallback(op) || op == Operation.REPORT ? status.getCode() : -1,
                op == Operation.BEGIN && success ? XID : null, op == Operation.REGISTER && success ? BRANCH_ID : 0L,
                op == Operation.LOCK && lockable, null);
    }

    private static AbstractTransactionResponse call(Operation op, InjectedHandler handler) {
        RpcContext ctx = new RpcContext();
        switch (op) {
            case BEGIN: return handler.handle(new GlobalBeginRequest(), ctx);
            case COMMIT: GlobalCommitRequest commit = new GlobalCommitRequest(); commit.setXid(XID); return handler.handle(commit, ctx);
            case ROLLBACK: GlobalRollbackRequest rollback = new GlobalRollbackRequest(); rollback.setXid(XID); return handler.handle(rollback, ctx);
            case STATUS: GlobalStatusRequest status = new GlobalStatusRequest(); status.setXid(XID); return handler.handle(status, ctx);
            case REPORT: GlobalReportRequest report = new GlobalReportRequest(); report.setXid(XID); report.setGlobalStatus(GlobalStatus.Rollbacking); return handler.handle(report, ctx);
            case REGISTER: BranchRegisterRequest register = new BranchRegisterRequest(); register.setXid(XID); return handler.handle(register, ctx);
            case BRANCH_REPORT: BranchReportRequest branch = new BranchReportRequest(); branch.setXid(XID); branch.setBranchId(BRANCH_ID); return handler.handle(branch, ctx);
            case LOCK: GlobalLockQueryRequest lock = new GlobalLockQueryRequest(); lock.setXid(XID); return handler.handle(lock, ctx);
            default: throw new IllegalArgumentException("unknown operation");
        }
    }

    /** 只注入核心操作的结果/异常，真实 AbstractTCInboundHandler 决定错误响应和回填。 */
    private static final class InjectedHandler extends AbstractTCInboundHandler {
        private Exception failure;
        private int calls;
        private boolean lockable = true;
        private GlobalStatus statusBeforeFailure;
        private void execute(Operation op, AbstractTransactionResponse response) throws TransactionException {
            calls++;
            if (Objects.nonNull(statusBeforeFailure) && response instanceof AbstractGlobalEndResponse) {
                ((AbstractGlobalEndResponse) response).setGlobalStatus(statusBeforeFailure);
            }
            if (failure instanceof TransactionException) { throw (TransactionException) failure; }
            if (failure instanceof RuntimeException) { throw (RuntimeException) failure; }
            if (Objects.nonNull(failure)) { throw new IllegalStateException(failure); }
        }
        @Override protected void doGlobalBegin(GlobalBeginRequest req, GlobalBeginResponse res, RpcContext ctx) throws TransactionException { execute(Operation.BEGIN, res); res.setXid(XID); }
        @Override protected void doGlobalCommit(GlobalCommitRequest req, GlobalCommitResponse res, RpcContext ctx) throws TransactionException { execute(Operation.COMMIT, res); res.setGlobalStatus(GlobalStatus.Committed); }
        @Override protected void doGlobalRollback(GlobalRollbackRequest req, GlobalRollbackResponse res, RpcContext ctx) throws TransactionException { execute(Operation.ROLLBACK, res); res.setGlobalStatus(GlobalStatus.Rollbacked); }
        @Override protected void doGlobalStatus(GlobalStatusRequest req, GlobalStatusResponse res, RpcContext ctx) throws TransactionException { execute(Operation.STATUS, res); res.setGlobalStatus(GlobalStatus.Begin); }
        @Override protected void doGlobalReport(GlobalReportRequest req, GlobalReportResponse res, RpcContext ctx) throws TransactionException { execute(Operation.REPORT, res); }
        @Override protected void doBranchRegister(BranchRegisterRequest req, BranchRegisterResponse res, RpcContext ctx) throws TransactionException { execute(Operation.REGISTER, res); res.setBranchId(BRANCH_ID); }
        @Override protected void doBranchReport(BranchReportRequest req, BranchReportResponse res, RpcContext ctx) throws TransactionException { execute(Operation.BRANCH_REPORT, res); }
        @Override protected void doLockCheck(GlobalLockQueryRequest req, GlobalLockQueryResponse res, RpcContext ctx) throws TransactionException { execute(Operation.LOCK, res); res.setLockable(lockable); }
    }
}
