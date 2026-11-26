import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.atomic.AtomicLong;
import org.junit.platform.engine.TestExecutionResult;
import org.junit.platform.engine.support.descriptor.ClassSource;
import org.junit.platform.engine.support.descriptor.MethodSource;
import org.junit.platform.engine.discovery.ClassNameFilter;
import org.junit.platform.engine.discovery.DiscoverySelectors;
import org.junit.platform.launcher.Launcher;
import org.junit.platform.launcher.TestExecutionListener;
import org.junit.platform.launcher.TestIdentifier;
import org.junit.platform.launcher.TestPlan;
import org.junit.platform.launcher.core.LauncherDiscoveryRequestBuilder;
import org.junit.platform.launcher.core.LauncherFactory;

/** 原JUnit引擎逐案例观察器；保留原测试、条件禁用和参数化运行身份。 */
public final class SourceJUnitRunner implements TestExecutionListener {
    private static final ObjectMapper JSON = new ObjectMapper();
    private final PrintWriter output;
    private final TestPlan plan;
    private final AtomicLong sequence = new AtomicLong();
    private long passed;
    private long failed;
    private long aborted;
    private long containerFailed;
    private long skipped;
    private SourceJUnitRunner(Path path, TestPlan original) throws Exception {
        output = new PrintWriter(Files.newBufferedWriter(path, StandardCharsets.UTF_8));
        plan = original;
    }
    /** 参数为模块测试类目录、原始事件文件和discover/execute模式；非零退出保持失败可见。 */
    public static void main(String[] args) throws Exception {
        Launcher launcher = LauncherFactory.create();
        TestPlan plan = launcher.discover(LauncherDiscoveryRequestBuilder.request()
            .selectors(DiscoverySelectors.selectClasspathRoots(Set.of(Path.of(args[0]))))
            .filters(ClassNameFilter.includeClassNamePatterns(".*"))
            .configurationParameter("junit.jupiter.execution.parallel.enabled", "false").build());
        SourceJUnitRunner listener = new SourceJUnitRunner(Path.of(args[1]), plan);
        Map<String, Object> runtime = new LinkedHashMap<>();
        runtime.put("kind", "runtime"); runtime.put("mode", args[2]);
        runtime.put("java_version", System.getProperty("java.version"));
        runtime.put("cwd", System.getProperty("user.dir"));
        runtime.put("user_home", System.getProperty("user.home"));
        runtime.put("tmpdir", System.getProperty("java.io.tmpdir"));
        runtime.put("classpath", System.getProperty("java.class.path"));
        runtime.put("hosts_file", System.getProperty("jdk.net.hosts.file"));
        runtime.put("launcher_origin", LauncherFactory.class.getProtectionDomain().getCodeSource().getLocation().toString());
        Class<?> engine = Class.forName("org.junit.jupiter.engine.JupiterTestEngine");
        runtime.put("jupiter_origin", engine.getProtectionDomain().getCodeSource().getLocation().toString());
        runtime.put("jupiter_version", engine.getPackage().getImplementationVersion());
        listener.emit(runtime);
        for (TestIdentifier root : plan.getRoots()) { listener.tree(root); }
        if (args[2].equals("execute")) { launcher.execute(plan, listener); }
        Map<String, Object> summary = new LinkedHashMap<>();
        summary.put("kind", "summary"); summary.put("mode", args[2]);
        summary.put("passed", listener.passed); summary.put("failed", listener.failed);
        summary.put("container_failed", listener.containerFailed);
        summary.put("aborted", listener.aborted); summary.put("skipped_test_events", listener.skipped);
        summary.put("guard_denials", HarnessGuard.DENIED.get());
        listener.emit(summary); listener.output.close();
        if (listener.failed > 0 || listener.containerFailed > 0 || HarnessGuard.DENIED.get() > 0) { System.exit(1); }
    }
    private void tree(TestIdentifier id) {
        event("discovered", id, Map.of());
        for (TestIdentifier child : plan.getChildren(id)) { tree(child); }
    }
    private void finalTree(TestPlan completed, TestIdentifier id) {
        event("final_descriptor", id, Map.of());
        for (TestIdentifier child : completed.getChildren(id)) { finalTree(completed, child); }
    }
    private synchronized void emit(Map<String, Object> row) {
        row.put("sequence", sequence.incrementAndGet());
        try { output.println(JSON.writeValueAsString(row)); output.flush(); if (output.checkError()) { throw new IllegalStateException("Original event output truncated"); } }
        catch (Exception error) { throw new IllegalStateException("Cannot preserve original JUnit observation", error); }
    }
    private void event(String kind, TestIdentifier id, Map<String, Object> extra) {
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("kind", kind); row.put("id", id.getUniqueId()); row.put("display", id.getDisplayName());
        row.put("test", id.isTest()); row.put("container", id.isContainer());
        row.put("parent", plan.getParent(id).map(TestIdentifier::getUniqueId).orElse(null));
        id.getSource().ifPresent(source -> {
            row.put("source_type", source.getClass().getName());
            if (source instanceof MethodSource) {
                MethodSource method = (MethodSource) source;
                row.put("class", method.getClassName()); row.put("method", method.getMethodName());
                row.put("parameters", method.getMethodParameterTypes());
                row.put("declaring_class", method.getJavaMethod().getDeclaringClass().getName());
            } else if (source instanceof ClassSource) { row.put("class", ((ClassSource) source).getClassName()); }
            else { row.put("source", source.toString()); }
        });
        row.putAll(extra); emit(row);
    }
    /** 保留真正动态注册的子案例。 */
    @Override public void dynamicTestRegistered(TestIdentifier id) { event("dynamic_registered", id, Map.of()); }
    /** 保留JUnit原条件的禁用原因，不合成成功结果。 */
    @Override public void executionSkipped(TestIdentifier id, String reason) {
        if (id.isTest()) { skipped++; } event("skipped", id, Map.of("reason", reason));
    }
    /** 记录原引擎实际开始事件。 */
    @Override public void executionStarted(TestIdentifier id) { event("started", id, Map.of()); }
    /** 记录原引擎真实结果及完整异常；container与具体案例分别保留。 */
    @Override public void executionFinished(TestIdentifier id, TestExecutionResult result) {
        if (id.isTest()) {
            switch (result.getStatus()) {
                case SUCCESSFUL: passed++; break;
                case FAILED: failed++; break;
                case ABORTED: aborted++; break;
                default: throw new IllegalStateException("Unexpected source test status");
            }
        }
        if (id.isContainer() && result.getStatus() == TestExecutionResult.Status.FAILED) { containerFailed++; }
        Map<String, Object> extra = new LinkedHashMap<>(); extra.put("status", result.getStatus().name());
        result.getThrowable().ifPresent(error -> {
            StringWriter text = new StringWriter(); error.printStackTrace(new PrintWriter(text));
            extra.put("error_type", error.getClass().getName()); extra.put("error", text.toString());
        });
        event("finished", id, extra);
    }
    /** 记录引擎计划结束，区别于进程退出或超时。 */
    @Override public void testPlanExecutionFinished(TestPlan completed) {
        for (TestIdentifier root : completed.getRoots()) { finalTree(completed, root); }
        emit(new LinkedHashMap<>(Map.of("kind", "plan_finished")));
    }
}
