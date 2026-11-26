package org.apache.seata.server.metrics;

import java.util.Arrays;
import java.util.stream.Stream;
import org.apache.seata.core.event.GlobalTransactionEvent;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.metrics.Id;
import org.apache.seata.metrics.IdConstants;
import org.apache.seata.metrics.registry.Registry;
import org.apache.seata.metrics.registry.compact.CompactRegistry;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.Arguments;
import org.junit.jupiter.params.provider.MethodSource;
import org.junit.jupiter.params.provider.NullSource;
import org.junit.jupiter.params.provider.ValueSource;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.spy;
import static org.mockito.Mockito.verify;

/** 使用实际CompactRegistry验证事件生命周期、真实计数和Timer样本。 */
public class MetricsLifecycleCandidateTest {
    private Registry registry;
    private MetricsSubscriber subscriber;

    @BeforeEach
    void prepare() {
        registry = spy(new CompactRegistry());
        registry.clearUp();
        subscriber = new MetricsSubscriber(registry);
    }

    @AfterEach
    void cleanup() {
        registry.clearUp();
    }

    private static String[] completionStatuses() {
        return new String[] {"Committed", "Rollbacked", "CommitFailed", "RollbackFailed", "TimeoutRollbacked",
                "TimeoutRollbackFailed", "CommitRetryTimeout", "RollbackRetryTimeout",
                IdConstants.STATUS_VALUE_AFTER_COMMITTED_KEY, IdConstants.STATUS_VALUE_AFTER_ROLLBACKED_KEY};
    }

    static Stream<Arguments> unfinishedStatuses() {
        return Arrays.stream(completionStatuses()).flatMap(status -> Stream.of(
                Arguments.of(status, false), Arguments.of(status, true)));
    }

    static Stream<Arguments> timedStatuses() {
        return Arrays.stream(completionStatuses()).filter(status -> !status.equals("TimeoutRollbacked"))
                .map(status -> Arguments.of(status));
    }

    private GlobalTransactionEvent event(String status, Long begin, Long end, boolean retry) {
        return new GlobalTransactionEvent(42, "tc", "owned-metrics", "owned-app", "owned-group",
                begin, end, status, retry, false);
    }

    private Id tagged(Id id) {
        return id.withTag(IdConstants.APP_ID_KEY, "owned-app").withTag(IdConstants.GROUP_KEY, "owned-group");
    }

    private Id timer(String status) {
        if (status.equals("Committed")) return MeterIdConstants.TIMER_COMMITTED;
        if (status.equals("Rollbacked")) return MeterIdConstants.TIMER_ROLLBACKED;
        if (status.equals(IdConstants.STATUS_VALUE_AFTER_COMMITTED_KEY)) return MeterIdConstants.TIMER_AFTER_COMMITTED;
        if (status.equals(IdConstants.STATUS_VALUE_AFTER_ROLLBACKED_KEY)) return MeterIdConstants.TIMER_AFTER_ROLLBACKED;
        return MeterIdConstants.TIMER_FAILED;
    }

    private Id summary(String status) {
        if (status.equals("Committed")) return MeterIdConstants.SUMMARY_COMMITTED;
        if (status.equals("Rollbacked")) return MeterIdConstants.SUMMARY_ROLLBACKED;
        if (status.equals(IdConstants.STATUS_VALUE_AFTER_COMMITTED_KEY)) return MeterIdConstants.SUMMARY_AFTER_COMMITTED;
        if (status.equals(IdConstants.STATUS_VALUE_AFTER_ROLLBACKED_KEY)) return MeterIdConstants.SUMMARY_AFTER_ROLLBACKED;
        return MeterIdConstants.SUMMARY_FAILED;
    }

    @ParameterizedTest
    @MethodSource("unfinishedStatuses")
    void unfinishedCompletionCannotChangeCompletionCountersOrInventLatency(String status, boolean retry) {
        subscriber.recordGlobalTransactionEventForMetrics(event("Begin", 10L, null, false));
        GlobalTransactionEvent ongoing = event(status, 10L, null, retry);
        assertDoesNotThrow(() -> subscriber.recordGlobalTransactionEventForMetrics(ongoing));
        assertEquals(1, registry.getCounter(tagged(MeterIdConstants.COUNTER_ACTIVE)).get());
        for (Id id : new Id[] {MeterIdConstants.COUNTER_COMMITTED, MeterIdConstants.COUNTER_ROLLBACKED,
                MeterIdConstants.COUNTER_AFTER_COMMITTED, MeterIdConstants.COUNTER_AFTER_ROLLBACKED}) {
            assertEquals(0, registry.getCounter(tagged(id)).get());
        }
        for (Id id : new Id[] {MeterIdConstants.SUMMARY_FAILED, MeterIdConstants.SUMMARY_TWO_PHASE_TIMEOUT,
                MeterIdConstants.SUMMARY_COMMITTED, MeterIdConstants.SUMMARY_ROLLBACKED,
                MeterIdConstants.SUMMARY_AFTER_COMMITTED, MeterIdConstants.SUMMARY_AFTER_ROLLBACKED}) {
            assertEquals(0, registry.getSummary(tagged(id)).count());
        }
        assertNull(ongoing.getEndTime());
        verify(registry, never()).getTimer(any());
    }

    @ParameterizedTest
    @MethodSource("timedStatuses")
    void completeEventRetainsActualLatencyAndCount(String status) {
        assertDoesNotThrow(() -> subscriber.recordGlobalTransactionEventForMetrics(event(status, 10L, 40L, false)));
        assertEquals(1, registry.getSummary(tagged(summary(status))).count());
        assertEquals(1, registry.getTimer(tagged(timer(status))).count());
        assertEquals(30000, registry.getTimer(tagged(timer(status))).total());
    }

    @ParameterizedTest
    @MethodSource("timedStatuses")
    void missingBeginRetainsCompletionCountWithoutSyntheticLatency(String status) {
        assertDoesNotThrow(() -> subscriber.recordGlobalTransactionEventForMetrics(event(status, null, 40L, false)));
        verify(registry, never()).getTimer(any());
        assertEquals(1, registry.getSummary(tagged(summary(status))).count());
    }

    @ParameterizedTest
    @MethodSource("timedStatuses")
    void reversedClockDoesNotCreateTimerSample(String status) {
        assertDoesNotThrow(() -> subscriber.recordGlobalTransactionEventForMetrics(event(status, 40L, 10L, false)));
        verify(registry, never()).getTimer(any());
        assertEquals(1, registry.getSummary(tagged(summary(status))).count());
    }

    @ParameterizedTest
    @MethodSource("timedStatuses")
    void overflowingClockDoesNotCreateTimerSample(String status) {
        assertDoesNotThrow(() -> subscriber.recordGlobalTransactionEventForMetrics(event(status, Long.MIN_VALUE, Long.MAX_VALUE, false)));
        verify(registry, never()).getTimer(any());
        assertEquals(1, registry.getSummary(tagged(summary(status))).count());
    }

    @ParameterizedTest
    @ValueSource(strings = {"CommitRetrying", "RollbackRetrying", "TimeoutRollbackRetrying"})
    void actualDoingRetriesRetainDetailedSummary(String status) {
        subscriber.recordGlobalTransactionEventForMetrics(event(status, 10L, null, true));
        Id id = status.equals("CommitRetrying") ? MeterIdConstants.SUMMARY_COMMIT_RETRYING
                : status.equals("RollbackRetrying") ? MeterIdConstants.SUMMARY_ROLLBACK_RETRYING
                : MeterIdConstants.SUMMARY_TIMEOUT_ROLLBACK_RETRYING;
        assertEquals(1, registry.getSummary(tagged(id).withTag(IdConstants.TRANSACTION_NAME_KEY, "owned-metrics")).count());
        verify(registry, never()).getTimer(any());
    }

    @Test
    void restoredTimeoutReentryDoesNotDecrementFreshProcessActiveCount() {
        assertDoesNotThrow(() -> subscriber.recordGlobalTransactionEventForMetrics(event("CommitRetryTimeout", 10L, null, true)));
        assertEquals(0, registry.getCounter(tagged(MeterIdConstants.COUNTER_ACTIVE)).get());
        assertEquals(0, registry.getSummary(tagged(MeterIdConstants.SUMMARY_FAILED)).count());
        subscriber.recordGlobalTransactionEventForMetrics(event(IdConstants.STATUS_VALUE_AFTER_COMMITTED_KEY, 10L, 40L, true));
        assertEquals(0, registry.getCounter(tagged(MeterIdConstants.COUNTER_ACTIVE)).get());
        assertEquals(1, registry.getCounter(tagged(MeterIdConstants.COUNTER_AFTER_COMMITTED)).get());
        assertEquals(1, registry.getTimer(tagged(MeterIdConstants.TIMER_AFTER_COMMITTED)).count());
    }

    @Test
    void actualZeroDurationIsAValidSample() {
        subscriber.recordGlobalTransactionEventForMetrics(event("Committed", 10L, 10L, false));
        assertEquals(1, registry.getTimer(tagged(MeterIdConstants.TIMER_COMMITTED)).count());
        assertEquals(0, registry.getTimer(tagged(MeterIdConstants.TIMER_COMMITTED)).total());
    }

    @Test
    void nullEventIsIgnored() {
        assertDoesNotThrow(() -> subscriber.recordGlobalTransactionEventForMetrics(null));
    }

    @Test
    void absentRegistryIsIgnored() {
        assertDoesNotThrow(() -> new MetricsSubscriber(null).recordGlobalTransactionEventForMetrics(null));
    }

    @ParameterizedTest
    @NullSource
    @ValueSource(strings = {"unknown"})
    void unknownStatusIsIgnored(String status) {
        assertDoesNotThrow(() -> subscriber.recordGlobalTransactionEventForMetrics(event(status, 10L, null, false)));
        assertEquals(0, registry.getCounter(tagged(MeterIdConstants.COUNTER_ACTIVE)).get());
    }
}
