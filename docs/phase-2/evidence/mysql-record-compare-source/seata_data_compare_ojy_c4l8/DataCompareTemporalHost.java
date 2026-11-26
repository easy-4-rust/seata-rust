import java.sql.Date;
import java.sql.Time;
import java.sql.Timestamp;
import java.time.LocalDateTime;

/** 固定 JDK 的日期时间转换原语，供 Rust 差分夹具使用。 */
public final class DataCompareTemporalHost {
    private DataCompareTemporalHost() {
    }

    /** 返回由原 JDK 解析的同类型值在固定时区下的比较表示。 */
    public static void main(String[] args) {
        if ("--digit-blocks".equals(args[0])) {
            for (int unit = 0; unit <= Character.MAX_VALUE; unit++) {
                if (Character.digit((char) unit, 10) == 0) {
                    System.out.printf("%04x%n", unit);
                }
            }
            return;
        }
        if ("--jdbc".equals(args[0])) {
            StringBuilder decoded = new StringBuilder();
            for (int offset = 0; offset < args[2].length(); offset += 4) {
                decoded.append((char) Integer.parseInt(args[2], offset, offset + 4, 16));
            }
            String input = decoded.toString();
            try {
                java.util.Date jdbc = switch (args[1]) {
                    case "date" -> Date.valueOf(input);
                    case "time" -> Time.valueOf(input);
                    case "timestamp" -> Timestamp.valueOf(input);
                    default -> throw new AssertionError("unknown JDBC kind");
                };
                long seconds = Math.floorDiv(jdbc.getTime(), 1000);
                int nanos = jdbc instanceof Timestamp timestamp ? timestamp.getNanos() : 0;
                System.out.printf("%d:%09d%n", seconds, nanos);
            } catch (IllegalArgumentException error) {
                System.out.println(error.getClass().getName());
            }
            return;
        }
        Object value = switch (args[0]) {
            case "date" -> Date.valueOf(args[1]);
            case "time" -> Time.valueOf(args[1]);
            case "timestamp" -> Timestamp.valueOf(args[1]);
            case "local" -> LocalDateTime.parse(args[1]);
            default -> throw new IllegalArgumentException("unknown temporal kind");
        };
        System.out.println(value);
    }
}
