package org.apache.seata.server.metrics;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.google.common.eventbus.Subscribe;
import com.sun.net.httpserver.HttpServer;
import java.net.InetSocketAddress;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CopyOnWriteArrayList;
import org.apache.seata.core.event.GlobalTransactionEvent;
import org.apache.seata.metrics.Id;
import org.apache.seata.metrics.IdConstants;
import org.apache.seata.metrics.registry.Registry;
import org.apache.seata.server.ServerApplication;
import org.apache.seata.server.event.EventBusManager;
import org.springframework.util.StringUtils;

/** 自有TC的只读事件/实际Registry观察入口，不替换producer、subscriber或业务实现。 */
public final class MetricsRollbackControl {
    private final Registry registry;
    private final List<Map<String, Object>> events = new CopyOnWriteArrayList<>();

    private MetricsRollbackControl(Registry registry) {
        this.registry = registry;
    }

    /** 收集原总线事件；参数为原事件，返回值无，不修改事件或指标。 */
    @Subscribe
    public void observe(GlobalTransactionEvent event) {
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("transaction_id", event.getId());
        row.put("role", event.getRole());
        row.put("name", event.getName());
        row.put("application_id", event.getApplicationId());
        row.put("group", event.getGroup());
        row.put("status", event.getStatus());
        row.put("begin_time", event.getBeginTime());
        row.put("end_time", event.getEndTime());
        row.put("retry_global", event.isRetryGlobal());
        row.put("retry_branch", event.isRetryBranch());
        events.add(row);
    }

    private Id tagged(Id id) {
        return id.withTag(IdConstants.APP_ID_KEY, "real-at-full-sdk").withTag(IdConstants.GROUP_KEY, "real_at");
    }

    private Map<String, Object> snapshot() {
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("pid", ProcessHandle.current().pid());
        out.put("registry_class", registry.getClass().getName());
        List<String> subscribers = new ArrayList<>();
        for (Object subscriber : EventBusManager.get().getSubscribers()) {
            if (subscriber.getClass().getName().equals(MetricsSubscriber.class.getName())) {
                subscribers.add(subscriber.getClass().getProtectionDomain().getCodeSource().getLocation().toString());
            }
        }
        out.put("subscriber_sources", subscribers);
        Map<String, Object> values = new LinkedHashMap<>();
        // 只读get/count/total；不调用measure，不清空或重置真实Registry窗口。
        values.put("active", registry.getCounter(tagged(MeterIdConstants.COUNTER_ACTIVE)).get());
        values.put("failed", registry.getSummary(tagged(MeterIdConstants.SUMMARY_FAILED)).count());
        values.put("two_phase_timeout", registry.getSummary(tagged(MeterIdConstants.SUMMARY_TWO_PHASE_TIMEOUT)).count());
        values.put("failed_timer_count", registry.getTimer(tagged(MeterIdConstants.TIMER_FAILED)).count());
        values.put("failed_timer_total", registry.getTimer(tagged(MeterIdConstants.TIMER_FAILED)).total());
        values.put("after_rollbacked", registry.getCounter(tagged(MeterIdConstants.COUNTER_AFTER_ROLLBACKED)).get());
        values.put("after_rollbacked_summary", registry.getSummary(tagged(MeterIdConstants.SUMMARY_AFTER_ROLLBACKED)).count());
        values.put("after_rollbacked_timer_count", registry.getTimer(tagged(MeterIdConstants.TIMER_AFTER_ROLLBACKED)).count());
        values.put("after_rollbacked_timer_total", registry.getTimer(tagged(MeterIdConstants.TIMER_AFTER_ROLLBACKED)).total());
        out.put("metrics", values);
        out.put("events", new ArrayList<>(events));
        return out;
    }

    /** 启动原TC并提供只读观察端点；参数为原TC参数，返回值无。 */
    public static void main(String[] args) throws Exception {
        Path directory = Path.of(System.getProperty("seata.it.metrics.dir")).toAbsolutePath().normalize();
        String token = System.getProperty("seata.it.metrics.token");
        int port = Integer.parseInt(System.getProperty("seata.it.metrics.port"));
        if (!StringUtils.hasText(token) || !token.matches("phase0-metrics-[a-f0-9]{8}") || port != 28095
                || !directory.toString().matches("^[DE]:[\\\\/].*")
                || !directory.toRealPath().toString().matches("^[DE]:[\\\\/].*")
                || !Files.readString(directory.resolve(".phase0-owned-metrics")).trim().equals(token)) {
            throw new IllegalArgumentException("Owned D/E metrics fixture identity required");
        }
        ServerApplication.main(args);
        Registry registry = MetricsManager.get().getRegistry();
        if (Objects.isNull(registry) || !registry.getClass().getName().equals("org.apache.seata.metrics.registry.compact.CompactRegistry")) {
            throw new IllegalStateException("Actual compact metrics registry required");
        }
        MetricsRollbackControl fixture = new MetricsRollbackControl(registry);
        EventBusManager.get().register(fixture);
        ObjectMapper mapper = new ObjectMapper();
        HttpServer control = HttpServer.create(new InetSocketAddress("127.0.0.1", port), 0);
        control.createContext("/state", exchange -> {
            if (!"GET".equals(exchange.getRequestMethod()) || !token.equals(exchange.getRequestHeaders().getFirst("X-Phase0-Owner"))) {
                exchange.sendResponseHeaders(403, -1);
                exchange.close();
                return;
            }
            byte[] body = mapper.writeValueAsBytes(fixture.snapshot());
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, body.length);
            exchange.getResponseBody().write(body);
            exchange.close();
        });
        control.start();
        System.out.println("METRICS_CONTROL_READY pid=" + ProcessHandle.current().pid() + " port=" + port);
    }
}
