package seata.verification;

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Objects;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.undo.AbstractUndoExecutor;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.junit.platform.engine.TestExecutionResult;
import org.junit.platform.engine.discovery.DiscoverySelectors;
import org.junit.platform.engine.support.descriptor.MethodSource;
import org.junit.platform.launcher.Launcher;
import org.junit.platform.launcher.TestExecutionListener;
import org.junit.platform.launcher.TestIdentifier;
import org.junit.platform.launcher.core.LauncherDiscoveryRequestBuilder;
import org.junit.platform.launcher.core.LauncherFactory;

/** 选择原 MySQL Undo 执行器测试类，由真实 JUnit 引擎执行并保存逐案例身份与结果。 */
public final class MysqlUndoExecutorJUnit implements TestExecutionListener {
    private final PrintWriter output;
    private int passed;
    private int failed;
    private int skipped;
    private MysqlUndoExecutorJUnit(Path path) throws Exception {
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
            MysqlUndoExecutorAssertions.start(method.getClassName() + "#" + method.getMethodName());
            emitOriginalInput(method.getClassName());
        }
    }
    private static List<List<Object>> values(TableRecords records) {
        List<List<Object>> result = new ArrayList<>();
        for (Row row : records.getRows()) {
            List<Object> fields = new ArrayList<>();
            for (Field field : row.getFields()) { fields.add(field.getValue()); }
            result.add(fields);
        }
        return result;
    }
    private static void emitOriginalInput(String className) {
        try {
            java.lang.reflect.Field executorField = Class.forName(className).getDeclaredField("executor");
            executorField.setAccessible(true);
            AbstractUndoExecutor executor = (AbstractUndoExecutor) executorField.get(null);
            SQLUndoLog log = executor.getSqlUndoLog();
            Map<String, Object> input = new LinkedHashMap<>();
            input.put("table_name", log.getTableName());
            input.put("sql_type", log.getSqlType().toString());
            input.put("primary_keys", log.getBeforeImage().getTableMeta().getPrimaryKeyOnlyName());
            List<Map<String, Object>> columns = new ArrayList<>();
            for (Field field : log.getBeforeImage().getRows().get(0).getFields()) {
                Map<String, Object> column = new LinkedHashMap<>();
                column.put("name", field.getName()); column.put("jdbc_type", field.getType());
                columns.add(column);
            }
            input.put("columns", columns);
            input.put("before_image", values(log.getBeforeImage()));
            input.put("after_image", values(log.getAfterImage()));
            System.out.println("MYSQL_UNDO_EXECUTOR_INPUT\t" + className + "\t" + new ObjectMapper().writeValueAsString(input));
        } catch (Exception error) { throw new IllegalStateException(error); }
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
        MysqlUndoExecutorJUnit listener = new MysqlUndoExecutorJUnit(Path.of(args[0]));
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
