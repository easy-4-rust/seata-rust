/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements.  See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License.  You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
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
import org.apache.seata.core.protocol.transaction.BranchRegisterRequest;
import org.apache.seata.core.protocol.transaction.BranchRegisterResponse;
import org.apache.seata.core.protocol.transaction.GlobalBeginRequest;
import org.apache.seata.core.protocol.transaction.GlobalBeginResponse;
import org.apache.seata.core.protocol.transaction.GlobalCommitRequest;
import org.apache.seata.core.protocol.transaction.GlobalCommitResponse;
import org.apache.seata.core.protocol.transaction.GlobalReportRequest;
import org.apache.seata.core.protocol.transaction.GlobalReportResponse;
import org.apache.seata.core.protocol.transaction.GlobalRollbackRequest;
import org.apache.seata.core.protocol.transaction.GlobalRollbackResponse;
import org.apache.seata.core.protocol.transaction.GlobalStatusRequest;
import org.apache.seata.core.protocol.transaction.GlobalStatusResponse;
import org.apache.seata.core.rpc.RpcContext;
import org.apache.seata.server.session.GlobalSession;
import org.apache.seata.server.session.SessionHolder;
import org.junit.jupiter.api.Test;
import org.mockito.MockedStatic;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.mockStatic;
import static org.mockito.Mockito.when;

class AbstractTCInboundHandlerExceptionTest {

    private static final String XID = "127.0.0.1:8091:1";

    @Test
    void globalBeginMapsTransactionExceptionToFailedResponse() {
        StubHandler handler = new StubHandler();
        handler.beginFailure = new TransactionException(TransactionExceptionCode.BeginFailed, "begin denied");

        GlobalBeginResponse response = handler.handle(new GlobalBeginRequest(), new RpcContext());

        assertEquals(ResultCode.Failed, response.getResultCode());
        assertEquals(TransactionExceptionCode.BeginFailed, response.getTransactionExceptionCode());
        assertEquals("TransactionException[begin denied]", response.getMsg());
    }

    @Test
    void globalBeginMapsRuntimeExceptionWithoutTransactionCode() {
        StubHandler handler = new StubHandler();
        handler.beginFailure = new IllegalStateException("begin crashed");

        GlobalBeginResponse response = handler.handle(new GlobalBeginRequest(), new RpcContext());

        assertEquals(ResultCode.Failed, response.getResultCode());
        assertEquals(TransactionExceptionCode.Unknown, response.getTransactionExceptionCode());
        assertEquals("RuntimeException[begin crashed]", response.getMsg());
    }

    @Test
    void globalBeginWrapsStoreExceptionAsFailedStore() {
        StubHandler handler = new StubHandler();
        handler.beginFailure = new StoreException("disk unavailable");

        GlobalBeginResponse response = handler.handle(new GlobalBeginRequest(), new RpcContext());

        assertEquals(ResultCode.Failed, response.getResultCode());
        assertEquals(TransactionExceptionCode.FailedStore, response.getTransactionExceptionCode());
        assertTrue(response.getMsg().contains("begin global request failed"));
    }

    @Test
    void globalCommitExceptionReturnsCurrentPersistedSessionStatus() {
        StubHandler handler = new StubHandler();
        handler.commitFailure = new TransactionException(TransactionExceptionCode.IO, "commit interrupted");
        GlobalSession session = mock(GlobalSession.class);
        when(session.getStatus()).thenReturn(GlobalStatus.CommitRetrying);

        try (MockedStatic<SessionHolder> sessions = mockStatic(SessionHolder.class)) {
            sessions.when(() -> SessionHolder.findGlobalSession(XID, false)).thenReturn(session);

            GlobalCommitRequest request = new GlobalCommitRequest();
            request.setXid(XID);
            GlobalCommitResponse response = handler.handle(request, new RpcContext());

            assertEquals(ResultCode.Failed, response.getResultCode());
            assertEquals(TransactionExceptionCode.IO, response.getTransactionExceptionCode());
            assertEquals(GlobalStatus.CommitRetrying, response.getGlobalStatus());
        }
    }

    @Test
    void globalCommitExceptionUsesFinishedWhenSessionDoesNotExist() {
        StubHandler handler = new StubHandler();
        handler.commitFailure = new IllegalStateException("commit crashed");

        try (MockedStatic<SessionHolder> sessions = mockStatic(SessionHolder.class)) {
            sessions.when(() -> SessionHolder.findGlobalSession(XID, false)).thenReturn(null);

            GlobalCommitRequest request = new GlobalCommitRequest();
            request.setXid(XID);
            GlobalCommitResponse response = handler.handle(request, new RpcContext());

            assertEquals(ResultCode.Failed, response.getResultCode());
            assertEquals(GlobalStatus.Finished, response.getGlobalStatus());
        }
    }

    @Test
    void globalStatusExceptionReturnsCurrentPersistedSessionStatus() {
        StubHandler handler = new StubHandler();
        handler.statusFailure = new TransactionException(TransactionExceptionCode.IO, "status lookup failed");
        GlobalSession session = mock(GlobalSession.class);
        when(session.getStatus()).thenReturn(GlobalStatus.RollbackRetrying);

        try (MockedStatic<SessionHolder> sessions = mockStatic(SessionHolder.class)) {
            sessions.when(() -> SessionHolder.findGlobalSession(XID, false)).thenReturn(session);

            GlobalStatusRequest request = new GlobalStatusRequest();
            request.setXid(XID);
            GlobalStatusResponse response = handler.handle(request, new RpcContext());

            assertEquals(ResultCode.Failed, response.getResultCode());
            assertEquals(GlobalStatus.RollbackRetrying, response.getGlobalStatus());
        }
    }

    @Test
    void globalReportRuntimeExceptionKeepsRequestedStatusWithoutSessionFallback() {
        StubHandler handler = new StubHandler();
        handler.reportFailure = new IllegalStateException("report failed");

        GlobalReportRequest request = new GlobalReportRequest();
        request.setXid(XID);
        request.setGlobalStatus(GlobalStatus.Rollbacking);
        GlobalReportResponse response = handler.handle(request, new RpcContext());

        assertEquals(ResultCode.Failed, response.getResultCode());
        assertEquals("RuntimeException[report failed]", response.getMsg());
        assertEquals(GlobalStatus.Rollbacking, response.getGlobalStatus());
    }

    @Test
    void branchRegisterStoreExceptionMapsToFailedStore() {
        StubHandler handler = new StubHandler();
        handler.branchRegisterFailure = new StoreException("branch persistence unavailable");

        BranchRegisterResponse response = handler.handle(new BranchRegisterRequest(), new RpcContext());

        assertEquals(ResultCode.Failed, response.getResultCode());
        assertEquals(TransactionExceptionCode.FailedStore, response.getTransactionExceptionCode());
        assertTrue(response.getMsg().contains("branch register request failed"));
    }

    private static final class StubHandler extends AbstractTCInboundHandler {
        private Exception beginFailure;
        private Exception commitFailure;
        private Exception statusFailure;
        private Exception reportFailure;
        private Exception branchRegisterFailure;

        @Override
        protected void doGlobalBegin(
                GlobalBeginRequest request, GlobalBeginResponse response, RpcContext rpcContext)
                throws TransactionException {
            throwIfConfigured(beginFailure);
        }

        @Override
        protected void doGlobalCommit(
                GlobalCommitRequest request, GlobalCommitResponse response, RpcContext rpcContext)
                throws TransactionException {
            throwIfConfigured(commitFailure);
        }

        @Override
        protected void doGlobalRollback(
                GlobalRollbackRequest request, GlobalRollbackResponse response, RpcContext rpcContext) {}

        @Override
        protected void doBranchRegister(
                BranchRegisterRequest request, BranchRegisterResponse response, RpcContext rpcContext)
                throws TransactionException {
            throwIfConfigured(branchRegisterFailure);
        }

        @Override
        protected void doBranchReport(
                org.apache.seata.core.protocol.transaction.BranchReportRequest request,
                org.apache.seata.core.protocol.transaction.BranchReportResponse response,
                RpcContext rpcContext) {}

        @Override
        protected void doLockCheck(
                org.apache.seata.core.protocol.transaction.GlobalLockQueryRequest request,
                org.apache.seata.core.protocol.transaction.GlobalLockQueryResponse response,
                RpcContext rpcContext) {}

        @Override
        protected void doGlobalStatus(
                GlobalStatusRequest request, GlobalStatusResponse response, RpcContext rpcContext)
                throws TransactionException {
            throwIfConfigured(statusFailure);
        }

        @Override
        protected void doGlobalReport(
                GlobalReportRequest request, GlobalReportResponse response, RpcContext rpcContext)
                throws TransactionException {
            throwIfConfigured(reportFailure);
        }

        private static void throwIfConfigured(Exception failure) throws TransactionException {
            if (failure instanceof TransactionException) {
                throw (TransactionException) failure;
            }
            if (failure instanceof RuntimeException) {
                throw (RuntimeException) failure;
            }
            if (failure != null) {
                throw new IllegalStateException(failure);
            }
        }
    }
}
