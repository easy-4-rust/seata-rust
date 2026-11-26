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
