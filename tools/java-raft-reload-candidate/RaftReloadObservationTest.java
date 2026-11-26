package org.apache.seata.server.cluster.raft;

import com.alipay.sofa.jraft.RouteTable;
import com.alipay.sofa.jraft.Status;
import com.alipay.sofa.jraft.conf.Configuration;
import org.apache.seata.common.store.SessionMode;
import org.apache.seata.server.cluster.raft.context.SeataClusterContext;
import org.apache.seata.server.session.SessionHolder;
import org.apache.seata.server.session.SessionManager;
import org.apache.seata.server.store.StoreConfig;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.MockedStatic;
import ch.qos.logback.classic.Logger;
import ch.qos.logback.classic.spi.ILoggingEvent;
import ch.qos.logback.core.read.ListAppender;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.doNothing;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.mockStatic;
import static org.mockito.Mockito.spy;
import static org.mockito.Mockito.when;

/** 观察真实 onLeaderStart 调度的 future 完成事件，覆盖成功、失败及交错任期。 */
public class RaftReloadObservationTest {
    private RaftStateMachine machine;
    private MockedStatic<RaftServerManager> servers;
    private MockedStatic<SessionHolder> sessions;
    private MockedStatic<CompletableFuture> futures;
    private final List<Runnable> actions = new ArrayList<>();
    private final List<CompletableFuture<Void>> pending = new ArrayList<>();

    @BeforeEach
    void setUp() {
        StoreConfig.setStartupParameter("file", "file", "file");
        machine = spy(new RaftStateMachine("reload-observation"));
        doNothing().when(machine).syncMetadata();
        RouteTable.getInstance().updateConfiguration("reload-observation", new Configuration());
        servers = mockStatic(RaftServerManager.class);
        servers.when(RaftServerManager::isRaftMode).thenReturn(true);
        sessions = mockStatic(SessionHolder.class);
        SessionManager manager = mock(SessionManager.class);
        when(manager.allSessions()).thenReturn(Collections.emptyList());
        sessions.when(SessionHolder::getRootSessionManager).thenReturn(manager);
        futures = mockStatic(CompletableFuture.class);
        futures.when(() -> CompletableFuture.runAsync(any(Runnable.class))).thenAnswer(call -> {
            actions.add(call.getArgument(0));
            CompletableFuture<Void> future = new CompletableFuture<>();
            pending.add(future);
            return future;
        });
    }

    @AfterEach
    void tearDown() {
        futures.close();
        sessions.close();
        servers.close();
        SeataClusterContext.unbindGroup();
    }

    private long completedTerm() throws Exception {
        try {
            return (long) RaftStateMachine.class.getMethod("getSessionReloadCompletedTerm").invoke(machine);
        } catch (NoSuchMethodException absentInOriginal) {
            return 0;
        }
    }

    private void succeed(int index) {
        actions.get(index).run();
        pending.get(index).complete(null);
    }

    @Test
    void pendingWorkDoesNotClaimCompletionOrChangeLeaderRouting() throws Exception {
        machine.onLeaderStart(3);
        assertEquals(0, completedTerm());
        assertTrue(machine.isLeader());
        assertEquals(1, actions.size());
    }

    @Test
    void successMarksCapturedTermAfterActualReloadAndUnbindsGroup() throws Exception {
        sessions.when(() -> SessionHolder.reload(Collections.emptyList(), SessionMode.RAFT, false))
                .thenAnswer(call -> {
                    assertEquals("reload-observation", SeataClusterContext.getGroup());
                    assertEquals(0, completedTerm());
                    return null;
                });
        machine.onLeaderStart(3);
        succeed(0);
        assertEquals(3, completedTerm());
        assertEquals(null, SeataClusterContext.getGroup());
    }

    @Test
    void exceptionalReloadNeverMarksSuccessAndUnbindsGroup() throws Exception {
        Logger logger = (Logger) LoggerFactory.getLogger(RaftStateMachine.class);
        ListAppender<ILoggingEvent> events = new ListAppender<>();
        events.start();
        logger.addAppender(events);
        try {
        sessions.when(() -> SessionHolder.reload(Collections.emptyList(), SessionMode.RAFT, false))
                .thenThrow(new IllegalStateException("owned reload failure"));
        machine.onLeaderStart(3);
        try {
            actions.get(0).run();
            throw new AssertionError("Expected actual reload failure");
        } catch (IllegalStateException expected) {
            pending.get(0).completeExceptionally(expected);
        }
        assertEquals(0, completedTerm());
        assertEquals(null, SeataClusterContext.getGroup());
        assertTrue(events.list.stream().anyMatch(event -> event.getFormattedMessage()
                .equals("session reload failed, groupId: reload-observation, term: 3")
                && Objects.nonNull(event.getThrowableProxy())
                && event.getThrowableProxy().getMessage().equals("owned reload failure")));
        assertFalse(events.list.stream().anyMatch(event -> event.getFormattedMessage()
                .equals("session reload completed, groupId: reload-observation, term: 3")));
        } finally {
            logger.detachAppender(events);
            events.stop();
        }
    }

    @Test
    void olderLateSuccessCannotOverwriteNewerSuccessfulTerm() throws Exception {
        machine.onLeaderStart(3);
        machine.onLeaderStop(Status.OK());
        machine.onLeaderStart(4);
        succeed(1);
        assertEquals(4, completedTerm());
        succeed(0);
        assertEquals(4, completedTerm());
    }

    @Test
    void failedNewTermCannotInheritPreviousSuccessAsCurrentCompletion() throws Exception {
        machine.onLeaderStart(3);
        succeed(0);
        machine.onLeaderStop(Status.OK());
        machine.onLeaderStart(4);
        pending.get(1).completeExceptionally(new IllegalStateException("next term failed"));
        assertEquals(3, completedTerm());
        assertEquals(4, machine.getCurrentTerm().get());
    }

    @Test
    void completionAfterStepDownDoesNotGrantLeadership() throws Exception {
        machine.onLeaderStart(3);
        machine.onLeaderStop(Status.OK());
        succeed(0);
        assertEquals(3, completedTerm());
        assertFalse(machine.isLeader());
    }

    @Test
    void nonRaftDoesNotScheduleOrClaimSessionReload() throws Exception {
        servers.when(RaftServerManager::isRaftMode).thenReturn(false);
        machine.onLeaderStart(3);
        assertEquals(0, actions.size());
        assertEquals(0, completedTerm());
    }

    @Test
    void duplicateLeaderNotificationPreservesExistingScheduling() throws Exception {
        machine.onLeaderStart(3);
        machine.onLeaderStart(3);
        assertEquals(1, actions.size());
        succeed(0);
        assertEquals(3, completedTerm());
    }
}
