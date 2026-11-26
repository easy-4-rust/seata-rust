package seata.verification;

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Objects;
import org.junit.platform.engine.TestExecutionResult;
import org.junit.platform.engine.discovery.DiscoverySelectors;
import org.junit.platform.engine.support.descriptor.MethodSource;
import org.junit.platform.launcher.Launcher;
import org.junit.platform.launcher.TestExecutionListener;
import org.junit.platform.launcher.TestIdentifier;
import org.junit.platform.launcher.core.LauncherDiscoveryRequestBuilder;
import org.junit.platform.launcher.core.LauncherFactory;

/** 选择四个原测试类，由真实 JUnit 引擎执行并保存逐案例身份与结果。 */
public final class RmMetadataJUnit implements TestExecutionListener {
    private final PrintWriter output;
    private int passed;
    private int failed;
    private int skipped;
    private RmMetadataJUnit(Path path) throws Exception {
        output = new PrintWriter(Files.newBufferedWriter(path, StandardCharsets.UTF_8));
    }
    private static String quote(String value) {
        if (Objects.isNull(value)) { return "null"; }
        StringBuilder text = new StringBuilder("\"");
        for (char c : value.toCharArray()) {
            if (c == '\\' || c == '\"') { text.append('\\').append(c); }
            else if (c < 32) { text.append(String.format("\\u%04x", (int)c)); }
            else { text.append(c); }
        }
        return text.append('\"').toString();
    }
    @Override public void executionStarted(TestIdentifier id) {
        if (id.isTest()) {
            MethodSource method = (MethodSource)id.getSource().orElseThrow();
            RmMetadataAssertions.start(method.getClassName() + "#" + method.getMethodName());
        }
    }
    @Override public void executionSkipped(TestIdentifier id, String reason) {
        if (id.isTest()) { skipped++; }
    }
    @Override public void executionFinished(TestIdentifier id, TestExecutionResult result) {
        if (!id.isTest()) {
            if (result.getStatus() == TestExecutionResult.Status.FAILED) { failed++; }
            return;
        }
        if (result.getStatus() == TestExecutionResult.Status.SUCCESSFUL) { passed++; }
        else { failed++; }
        MethodSource source = (MethodSource)id.getSource().orElseThrow();
        output.println("{\"kind\":\"case\",\"case_id\":" + quote(id.getUniqueId()) +
            ",\"class\":" + quote(source.getClassName()) + ",\"method\":" + quote(source.getMethodName()) +
            ",\"status\":" + quote(result.getStatus().name()) + "}");
        result.getThrowable().ifPresent(Throwable::printStackTrace);
        output.flush();
    }
    public static void main(String[] args) throws Exception {
        RmMetadataJUnit listener = new RmMetadataJUnit(Path.of(args[0]));
        LauncherDiscoveryRequestBuilder request = LauncherDiscoveryRequestBuilder.request()
            .configurationParameter("junit.jupiter.execution.parallel.enabled", "false");
        for (int i=1; i<args.length; i++) {
            request.selectors(DiscoverySelectors.selectClass(args[i]));
        }
        Launcher launcher = LauncherFactory.create();
        launcher.execute(request.build(), listener);
        listener.output.println("{\"kind\":\"summary\",\"passed\":" + listener.passed +
            ",\"failed\":" + listener.failed + ",\"skipped\":" + listener.skipped + "}");
        listener.output.close();
        if (listener.failed != 0 || listener.skipped != 0) { System.exit(1); }
    }
}
