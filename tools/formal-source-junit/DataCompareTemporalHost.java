import java.sql.Date;
import java.sql.Time;
import java.sql.Timestamp;
import java.time.LocalDateTime;
import java.time.format.DateTimeParseException;
import java.util.Locale;

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
        if ("--local".equals(args[0])) {
            try {
                LocalDateTime local = LocalDateTime.parse(decode(args[1]));
                System.out.printf(Locale.ROOT, "%d:%02d:%02d:%02d:%02d:%02d:%09d%n",
                        local.getYear(), local.getMonthValue(), local.getDayOfMonth(),
                        local.getHour(), local.getMinute(), local.getSecond(), local.getNano());
            } catch (DateTimeParseException error) {
                System.out.println(error.getClass().getName());
            }
            return;
        }
        if ("--jdbc".equals(args[0])) {
            String input = decode(args[2]);
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
        throw new IllegalArgumentException("unknown temporal probe");
    }

    private static String decode(String units) {
        StringBuilder decoded = new StringBuilder();
        for (int offset = 0; offset < units.length(); offset += 4) {
            decoded.append((char) Integer.parseInt(units, offset, offset + 4, 16));
        }
        return decoded.toString();
    }
}
