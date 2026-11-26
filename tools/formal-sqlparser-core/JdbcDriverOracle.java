import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.apache.seata.rm.datasource.util.JdbcUtils;

public final class JdbcDriverOracle {
    private static String hex(String value) {
        if (value == null) return "null";
        StringBuilder out = new StringBuilder();
        for (char unit : value.toCharArray()) out.append(String.format(Locale.ROOT, "%04x", (int)unit));
        return out.toString();
    }

    private static String decode(String value) {
        if (value.equals("null")) return null;
        char[] units = new char[value.length() / 4];
        for (int i = 0; i < units.length; i++) {
            units[i] = (char)Integer.parseInt(value.substring(i * 4, i * 4 + 4), 16);
        }
        return new String(units);
    }

    public static void main(String[] args) throws Exception {
        boolean present = args[0].equals("present");
        Thread thread = Thread.currentThread();
        ClassLoader original = thread.getContextClassLoader();
        List<String> rows = new ArrayList<>();
        try (URLClassLoader markers = new URLClassLoader(new URL[]{Path.of(args[3]).toUri().toURL()}, original)) {
            thread.setContextClassLoader(present ? markers : original);
            for (String input : Files.readAllLines(Path.of(args[1]))) {
                String url = decode(input);
                String driver;
                try {
                    driver = hex(com.alibaba.druid.util.JdbcUtils.getDriverClassName(url));
                } catch (SQLException error) {
                    driver = error.getClass().getName() + ":" + hex(error.getMessage());
                }
                String resource;
                try {
                    String id = JdbcUtils.buildResourceId(url);
                    resource = hex(id) + ":" + (id == url);
                } catch (NullPointerException error) {
                    resource = error.getClass().getName();
                }
                rows.add(input + "\t" + driver + "\t" + resource);
            }
            thread.setContextClassLoader(present ? original : markers);
            rows.add("cached\t" + com.alibaba.druid.util.JdbcUtils.getDriverClassName("jdbc:mysql:") +
                    "\t" + com.alibaba.druid.util.JdbcUtils.getDriverClassName("jdbc:clickhouse:"));
        } finally {
            thread.setContextClassLoader(original);
        }
        Files.write(Path.of(args[2]), rows);
    }
}
