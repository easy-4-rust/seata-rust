package org.apache.seata.server.cluster.raft.execute.branch;

import com.alipay.sofa.jraft.Closure;
import com.alipay.sofa.jraft.Status;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.CompletableFuture;
import java.util.stream.Stream;
import org.apache.seata.core.model.BranchStatus;
import org.apache.seata.core.model.BranchType;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.server.store.StoreConfig;
import org.apache.seata.server.cluster.raft.serializer.JacksonSerializer;
import org.apache.seata.server.cluster.raft.sync.RaftSyncMessageSerializer;
import org.apache.seata.server.cluster.raft.sync.msg.RaftBranchSessionSyncMsg;
import org.apache.seata.server.cluster.raft.sync.msg.RaftSyncMessage;
import org.apache.seata.server.cluster.raft.sync.msg.RaftSyncMsgType;
import org.apache.seata.server.cluster.raft.sync.msg.dto.BranchTransactionDTO;
import org.apache.seata.server.cluster.raft.util.RaftTaskUtil;
import org.apache.seata.server.session.BranchSession;
import org.apache.seata.server.session.GlobalSession;
import org.apache.seata.server.session.SessionHolder;
import org.apache.seata.server.storage.raft.session.RaftSessionManager;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.Arguments;
import org.junit.jupiter.params.provider.MethodSource;
import org.junit.jupiter.params.provider.NullAndEmptySource;
import org.junit.jupiter.params.provider.ValueSource;
import org.mockito.ArgumentMatchers;
import org.mockito.MockedStatic;
import org.mockito.Mockito;
import org.springframework.mock.env.MockEnvironment;

/** 通过真实sender/DTO序列化/follower分支对象验证上下文更新，原类也运行相同测试字节。 */
public class RaftContextReplicationTest {
    private static final String XID = "127.0.0.1:28291:710001";
    private static final long BRANCH = 710002L;

    @BeforeAll
    static void initializeActualConfigurationProvider() {
        MockEnvironment environment = new MockEnvironment().withProperty("seata.config.type", "file")
                .withProperty("seata.config.file.name", "file")
                .withProperty("seata.store.file.dir", Path.of(System.getProperty("user.home"), "store").toString());
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT, environment);
        StoreConfig.setStartupParameter("raft", "raft", "raft");
    }

    private static Object marker(BranchTransactionDTO dto) throws Exception {
        try {
            return dto.getClass().getMethod("getApplicationDataUpdated").invoke(dto);
        } catch (NoSuchMethodException missingOriginalFeature) {
            return null;
        }
    }

    private static void mark(BranchTransactionDTO dto, Boolean updated) throws Exception {
        try {
            Method method = dto.getClass().getMethod("setApplicationDataUpdated", Boolean.class);
            method.invoke(dto, updated);
        } catch (NoSuchMethodException missingOriginalFeature) {
            // 同字节原类RED：缺少显式更新能力时保持其实际旧行为，不伪造新属性。
        }
    }

    private static BranchSession branch(String context) {
        BranchSession branch = new BranchSession(BranchType.TCC);
        branch.setXid(XID);
        branch.setBranchId(BRANCH);
        branch.setStatus(BranchStatus.Registered);
        branch.setApplicationData(context);
        return branch;
    }

    private static BranchTransactionDTO dto(String context, Boolean updated) throws Exception {
        BranchTransactionDTO dto = new BranchTransactionDTO(XID, BRANCH);
        dto.setStatus(BranchStatus.PhaseOne_Done.getCode());
        dto.setApplicationData(context);
        mark(dto, updated);
        return dto;
    }

    private static BranchSession apply(String previous, BranchTransactionDTO dto) throws Throwable {
        BranchSession actual = branch(previous);
        GlobalSession session = new GlobalSession("fixture", "group", "context", 600000);
        session.setXid(XID);
        session.add(actual);
        RaftSessionManager manager = Mockito.mock(RaftSessionManager.class);
        Mockito.when(manager.findGlobalSession(XID)).thenReturn(session);
        try (MockedStatic<SessionHolder> holder = Mockito.mockStatic(SessionHolder.class)) {
            holder.when(() -> SessionHolder.getRootSessionManager("default")).thenReturn(manager);
            Assertions.assertEquals(Boolean.TRUE, new UpdateBranchSessionExecute().execute(
                    new RaftBranchSessionSyncMsg(RaftSyncMsgType.UPDATE_BRANCH_SESSION_STATUS, dto)));
        }
        Assertions.assertEquals(BranchStatus.PhaseOne_Done, actual.getStatus());
        Assertions.assertEquals(XID, actual.getXid());
        Assertions.assertEquals(BRANCH, actual.getBranchId());
        return actual;
    }

    @ParameterizedTest
    @NullAndEmptySource
    @ValueSource(strings = {"{\"actionContext\":{\"token\":\"prepared\"}}"})
    void senderQueuesExplicitCurrentContext(String context) throws Exception {
        BranchSession actual = branch(context);
        GlobalSession session = Mockito.mock(GlobalSession.class);
        Mockito.when(session.getXid()).thenReturn(XID);
        RaftSessionManager sender = Mockito.mock(RaftSessionManager.class, Mockito.CALLS_REAL_METHODS);
        try (MockedStatic<RaftTaskUtil> tasks = Mockito.mockStatic(RaftTaskUtil.class)) {
            tasks.when(() -> RaftTaskUtil.createTask(ArgumentMatchers.any(Closure.class),
                    ArgumentMatchers.any(), ArgumentMatchers.any(CompletableFuture.class))).thenAnswer(invocation -> {
                        RaftBranchSessionSyncMsg message = invocation.getArgument(1);
                        BranchTransactionDTO queued = message.getBranchSession();
                        Assertions.assertEquals(context, queued.getApplicationData());
                        Assertions.assertEquals(Boolean.TRUE, marker(queued));
                        Assertions.assertEquals(XID, queued.getXid());
                        Assertions.assertEquals(BRANCH, queued.getBranchId());
                        Assertions.assertEquals(BranchStatus.PhaseOne_Done.getCode(), queued.getStatus());
                        actual.setApplicationData("later-local-change");
                        Assertions.assertEquals(context, queued.getApplicationData());
                        Closure closure = invocation.getArgument(0);
                        closure.run(Status.OK());
                        return Boolean.TRUE;
                    });
            sender.onBranchStatusChange(session, actual, BranchStatus.PhaseOne_Done);
            Assertions.assertEquals(BranchStatus.PhaseOne_Done, actual.getStatus());
        }
    }

    static Stream<Arguments> changes() {
        return Stream.of(Arguments.of("before", "updated"), Arguments.of("before", ""),
                Arguments.of("before", null), Arguments.of(null, "updated"), Arguments.of("", null),
                Arguments.of("same", "same"));
    }

    @ParameterizedTest
    @MethodSource("changes")
    void followerAppliesExplicitContextIncludingNull(String previous, String updated) throws Throwable {
        Assertions.assertEquals(updated, apply(previous, dto(updated, true)).getApplicationData());
    }

    @ParameterizedTest
    @NullAndEmptySource
    @ValueSource(strings = {"original-context"})
    void legacyStatusOnlyPreservesContext(String previous) throws Throwable {
        Assertions.assertEquals(previous, apply(previous, dto(null, null)).getApplicationData());
    }

    @Test
    void explicitFalsePreservesContext() throws Throwable {
        Assertions.assertEquals("original", apply("original", dto("ignored", false)).getApplicationData());
    }

    @Test
    void legacyNonNullFieldWithoutMarkerKeepsOriginalStatusOnlyMeaning() throws Throwable {
        Assertions.assertEquals("original", apply("original", dto("ignored", null)).getApplicationData());
    }

    @Test
    void actualOriginalWireDecodesAndAppliesWithoutErasingContext() throws Throwable {
        byte[] bytes = Files.readAllBytes(Path.of(System.getProperty("seata.context.legacy.blob")));
        RaftSyncMessage decoded = RaftSyncMessageSerializer.decode(bytes);
        BranchTransactionDTO historical = ((RaftBranchSessionSyncMsg) decoded.getBody()).getBranchSession();
        Assertions.assertNull(marker(historical));
        Assertions.assertEquals("legacy-kept", apply("legacy-kept", historical).getApplicationData());
    }

    @ParameterizedTest
    @NullAndEmptySource
    @ValueSource(strings = {"{\"actionContext\":{\"token\":\"prepared\"}}"})
    void actualRaftSerializationPreservesExplicitUpdate(String context) throws Exception {
        RaftSyncMessage wrapper = new RaftSyncMessage();
        wrapper.setBody(new RaftBranchSessionSyncMsg(RaftSyncMsgType.UPDATE_BRANCH_SESSION_STATUS, dto(context, true)));
        RaftSyncMessage decoded = RaftSyncMessageSerializer.decode(RaftSyncMessageSerializer.encode(wrapper));
        BranchTransactionDTO actual = ((RaftBranchSessionSyncMsg) decoded.getBody()).getBranchSession();
        Assertions.assertEquals(context, actual.getApplicationData());
        Assertions.assertEquals(Boolean.TRUE, marker(actual));
    }

    @Test
    void unsetMarkerDoesNotAddNullPropertyToOrdinaryDto() {
        byte[] encoded = new JacksonSerializer().serialize(new BranchTransactionDTO(XID, BRANCH));
        com.fasterxml.jackson.databind.JsonNode outer;
        try {
            outer = new com.fasterxml.jackson.databind.ObjectMapper().readTree(encoded);
            Assertions.assertFalse(new String(outer.get("obj").binaryValue(), StandardCharsets.UTF_8)
                    .contains("applicationDataUpdated"));
        } catch (Exception error) {
            Assertions.fail(error);
        }
    }

    @Test
    void missingGlobalSessionDoesNotCreateOne() throws Throwable {
        RaftSessionManager manager = Mockito.mock(RaftSessionManager.class);
        try (MockedStatic<SessionHolder> holder = Mockito.mockStatic(SessionHolder.class)) {
            holder.when(() -> SessionHolder.getRootSessionManager("default")).thenReturn(manager);
            Assertions.assertEquals(Boolean.FALSE, new UpdateBranchSessionExecute().execute(
                    new RaftBranchSessionSyncMsg(RaftSyncMsgType.UPDATE_BRANCH_SESSION_STATUS, dto("new", true))));
        }
    }

    @Test
    void missingBranchDoesNotCreateOne() throws Throwable {
        GlobalSession session = new GlobalSession("fixture", "group", "context", 600000);
        session.setXid(XID);
        RaftSessionManager manager = Mockito.mock(RaftSessionManager.class);
        Mockito.when(manager.findGlobalSession(XID)).thenReturn(session);
        try (MockedStatic<SessionHolder> holder = Mockito.mockStatic(SessionHolder.class)) {
            holder.when(() -> SessionHolder.getRootSessionManager("default")).thenReturn(manager);
            Assertions.assertEquals(Boolean.FALSE, new UpdateBranchSessionExecute().execute(
                    new RaftBranchSessionSyncMsg(RaftSyncMsgType.UPDATE_BRANCH_SESSION_STATUS, dto("new", true))));
        }
        Assertions.assertTrue(session.getBranchSessions().isEmpty());
    }
}
