import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.Permission;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicLong;

/** 原测试执行器的环境边界；测试扩展，不替换任何原Seata断言。 */
@SuppressWarnings("removal")
public final class HarnessGuard extends SecurityManager {
    static final AtomicLong DENIED = new AtomicLong();
    private static final ObjectMapper JSON = new ObjectMapper();
    private final Path work;
    private final Path audit;
    private final Path scope;
    /** 无参数；从显式D盘启动属性取得隔离目录和审计输出。 */
    public HarnessGuard() {
        work = Path.of(Objects.requireNonNull(System.getProperty("seata.junit.work"))).toAbsolutePath().normalize();
        scope = Path.of(Objects.requireNonNull(System.getProperty("seata.junit.scope"))).toAbsolutePath().normalize();
        audit = work.resolve("guard-denials.jsonl");
    }
    /** 允许不涉及文件写入或网络的原反射、线程和测试操作。 */
    @Override public void checkPermission(Permission permission) {}
    /** 与普通权限检查一致，保留原JMX等非I/O上下文权限；文件和网络仍走明确边界。 */
    @Override public void checkPermission(Permission permission, Object context) { checkPermission(permission); }
    /** 核对真实文件写入位于本次隔离目录或D盘临时目录。 */
    @Override public void checkWrite(String file) { checkPath(file); }
    /** 核对原测试清理操作只能修改本次隔离目录或D盘临时目录。 */
    @Override public void checkDelete(String file) { checkPath(file); }
    private void checkPath(String file) {
        Path path = Path.of(file).toAbsolutePath().normalize();
        if (!path.startsWith(scope) && !path.startsWith(Path.of("D:/Temp").toAbsolutePath().normalize())) {
            deny("filesystem-write", path.toString());
        }
    }
    /** DNS只允许由本次启动的受控hosts文件解析；真实连接须显式提供自有夹具后才能放行。 */
    @Override public void checkConnect(String host, int port) {
        if (port == -1 && Objects.nonNull(System.getProperty("jdk.net.hosts.file"))) { return; }
        deny("network-connect", host + ":" + port);
    }
    /** 与双参数连接检查保持相同边界，不使用上下文绕过。 */
    @Override public void checkConnect(String host, int port, Object context) { checkConnect(host, port); }
    /** 本轮原单元测试尚未声明自有监听器，任何真实监听均留审计并拒绝。 */
    @Override public void checkListen(int port) { deny("network-listen", Integer.toString(port)); }
    private void deny(String kind, String target) {
        DENIED.incrementAndGet();
        try {
            synchronized (HarnessGuard.class) {
                Files.writeString(audit, JSON.writeValueAsString(Map.of("kind", kind, "target", target,
                    "thread", Thread.currentThread().getName())) + System.lineSeparator(),
                    StandardOpenOption.CREATE, StandardOpenOption.APPEND);
            }
        } catch (IOException error) { throw new SecurityException("Cannot persist guard denial", error); }
        throw new SecurityException("Source JUnit isolation refused " + kind + " " + target);
    }
}
