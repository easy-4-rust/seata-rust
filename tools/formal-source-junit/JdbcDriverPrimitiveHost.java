import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.sql.Driver;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Objects;

/** JVM 测试宿主只提供反射原语。Rust 负责驱动加载和异常包装的流程。 */
public final class JdbcDriverPrimitiveHost {
    private JdbcDriverPrimitiveHost() {
    }

    /** 从标准输入读取反射请求，以逐行协议输出实际结果。 */
    @SuppressWarnings("deprecation")
    public static void main(String[] args) throws Exception {
        BufferedReader input = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        Class<?> selected = null;
        String line;
        while (Objects.nonNull(line = input.readLine())) {
            String[] fields = line.split("\t", -1);
            if ("stop".equals(fields[0])) {
                return;
            }
            try {
                if ("instantiate".equals(fields[0])) {
                    Driver driver = (Driver) Objects.requireNonNull(selected).newInstance();
                    System.out.println("driver\t" + driver.getClass().getName() + "\t" + (driver instanceof org.h2.Driver));
                } else {
                    String name = decode(fields[1]);
                    if ("context".equals(fields[0])) {
                        ClassLoader loader = Thread.currentThread().getContextClassLoader();
                        selected = Objects.isNull(loader) ? null : loader.loadClass(name);
                    } else if ("default".equals(fields[0])) {
                        selected = Class.forName(name);
                    } else {
                        throw new IllegalArgumentException("unknown primitive");
                    }
                    System.out.println(Objects.isNull(selected) ? "none" : "class");
                }
            } catch (Throwable error) {
                List<String> parents = new ArrayList<>();
                for (Class<?> c = error.getClass().getSuperclass(); Objects.nonNull(c); c = c.getSuperclass()) {
                    parents.add(c.getName());
                }
                String message = error.getMessage();
                System.out.println("error\t" + error.getClass().getName() + "\t" + String.join(",", parents)
                        + "\t" + (Objects.isNull(message) ? "-" : HexFormat.of().formatHex(message.getBytes(StandardCharsets.UTF_8))));
            }
        }
    }

    private static String decode(String payload) {
        if ("-".equals(payload)) {
            return null;
        }
        char[] name = new char[payload.length() / 4];
        for (int i = 0; i < name.length; i++) {
            name[i] = (char) Integer.parseInt(payload.substring(4 * i, 4 * i + 4), 16);
        }
        return new String(name);
    }
}
