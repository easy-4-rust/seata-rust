import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;

public final class JdbcDriverReentryOracle {
    public static void main(String[] args) throws Exception {
        List<String> rows = new ArrayList<>();
        Thread thread = Thread.currentThread();
        ClassLoader original = thread.getContextClassLoader();
        try (URLClassLoader markers = new URLClassLoader(new URL[]{Path.of(args[1]).toUri().toURL()}, original)) {
            thread.setContextClassLoader(new ClassLoader(original) {
                boolean inside;

                @Override public Class<?> loadClass(String name) throws ClassNotFoundException {
                    String url = switch (name) {
                        case "com.mysql.cj.jdbc.Driver" -> "jdbc:mysql:";
                        case "com.clickhouse.jdbc.ClickHouseDriver" -> "jdbc:clickhouse:";
                        default -> null;
                    };
                    if (url == null) return super.loadClass(name);
                    if (inside) throw new ClassNotFoundException(name);
                    inside = true;
                    try {
                        rows.add(name + "\tinner\t" + com.alibaba.druid.util.JdbcUtils.getDriverClassName(url));
                        return markers.loadClass(name);
                    } catch (SQLException error) {
                        throw new IllegalStateException(error);
                    } finally {
                        inside = false;
                    }
                }
            });
            for (String url : new String[]{"jdbc:mysql:", "jdbc:clickhouse:"}) {
                String first = com.alibaba.druid.util.JdbcUtils.getDriverClassName(url);
                rows.add(url + "\touter\t" + first);
                rows.add(url + "\tlater\t" + com.alibaba.druid.util.JdbcUtils.getDriverClassName(url));
            }
        } finally {
            thread.setContextClassLoader(original);
        }
        Files.write(Path.of(args[0]), rows);
    }
}
