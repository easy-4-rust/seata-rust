import java.lang.instrument.Instrumentation;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;

/** 只观察每个真实JVM的隔离环境，不转换类、不修改原测试；测试扩展。 */
public final class SourceJvmEnvironmentProbe {
    private SourceJvmEnvironmentProbe() {}
    /** 参数为本次D盘审计目录和未使用的Instrumentation；启动时记录原JVM属性。 */
    public static void premain(String directory, Instrumentation ignored) throws Exception {
        Path root = Path.of(directory).toAbsolutePath().normalize();
        if (!root.startsWith(Path.of("D:/codex-builds").toAbsolutePath().normalize())) {
            throw new IllegalArgumentException("Source JVM audit escaped owned D workspace");
        }
        Files.createDirectories(root);
        Properties snapshot = new Properties();
        for (String key : new String[]{"user.home", "user.dir", "java.io.tmpdir", "jdk.net.hosts.file",
            "java.class.path", "java.version", "sun.java.command", "seata.fastjson2.concurrentRef.threads",
            "seata.fastjson2.concurrentRef.rounds"}) {
            snapshot.setProperty(key, System.getProperty(key, "<unset>"));
        }
        ProcessHandle process = ProcessHandle.current();
        snapshot.setProperty("pid", Long.toString(process.pid()));
        snapshot.setProperty("parent_pid", Long.toString(process.parent().map(ProcessHandle::pid).orElse(0L)));
        snapshot.setProperty("start_instant", process.info().startInstant().map(Object::toString).orElse("<unknown>"));
        snapshot.setProperty("command_line", process.info().commandLine().orElse("<unknown>"));
        snapshot.setProperty("JAVA_TOOL_OPTIONS", System.getenv().getOrDefault("JAVA_TOOL_OPTIONS", "<unset>"));
        try (java.io.OutputStream output = Files.newOutputStream(root.resolve(process.pid() + ".xml"))) {
            snapshot.storeToXML(output, "Observed source JVM environment; no class transformation", "UTF-8");
        }
    }
}
