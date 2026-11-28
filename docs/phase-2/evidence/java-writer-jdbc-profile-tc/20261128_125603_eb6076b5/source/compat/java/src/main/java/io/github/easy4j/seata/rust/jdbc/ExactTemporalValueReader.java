package io.github.easy4j.seata.rust.jdbc;

import com.mysql.cj.jdbc.result.ResultSetInternalMethods;
import com.mysql.cj.protocol.ResultsetRow;
import com.mysql.cj.protocol.ResultsetRows;
import java.nio.charset.StandardCharsets;
import java.sql.ResultSet;
import java.sql.SQLDataException;
import java.sql.SQLException;
import java.sql.SQLFeatureNotSupportedException;
import java.sql.Types;
import java.time.DateTimeException;
import java.time.LocalDate;
import java.util.Locale;
import java.util.Objects;

/**
 * Connector/J 8.0.27 缓冲结果集的无损 DATE/TIME 读取边界。
 * 保留真实 JDBC 编号，由调用方显式选择 String 值配置；不修改 Seata Undo parser。
 * 同时读取文本行和二进制行，避免 java.sql.Date/Time 与 getString 的历法、精度及负零时长损失。
 */
public final class ExactTemporalValueReader {
    private static final long MAX_TIME_MICROS = 3020399000000L;

    private ExactTemporalValueReader() {}

    /** 返回精确日历/时长字符串或 SQL NULL；其他 JDBC 类型保持原 getObject 行为。 */
    public static Object read(ResultSet rows, int index) throws SQLException {
        int jdbc = rows.getMetaData().getColumnType(index);
        if (jdbc != Types.DATE && jdbc != Types.TIME) return rows.getObject(index);
        byte[] raw = rows.getBytes(index);
        if (Objects.isNull(raw)) return null;
        ResultsetRows container = rows.unwrap(ResultSetInternalMethods.class).getRows();
        if (container.isDynamic()) throw new SQLFeatureNotSupportedException("Exact temporal profile requires buffered Connector/J rows");
        Object current = container.get(container.getPosition());
        if (!(current instanceof ResultsetRow)) throw new SQLFeatureNotSupportedException("Unknown Connector/J row encoding");
        // JDBC getScale 对 TIME 返回数值域的 0；物理 FSP 必须读取原始字段描述。
        int scale = rows.unwrap(ResultSetInternalMethods.class).getColumnDefinition().getFields()[index - 1].getDecimals();
        if (((ResultsetRow) current).isBinaryEncoded()) {
            return jdbc == Types.DATE ? binaryDate(raw) : binaryTime(raw, scale);
        }
        for (byte value : raw) if (value < 0) throw invalid();
        String text = new String(raw, StandardCharsets.US_ASCII);
        return jdbc == Types.DATE ? date(text) : time(text, scale);
    }

    private static String binaryDate(byte[] raw) throws SQLException {
        if (raw.length != 4) throw invalid();
        return date(String.format(Locale.ROOT, "%04d-%02d-%02d", little(raw, 0, 2), unsigned(raw[2]), unsigned(raw[3])));
    }

    private static String date(String text) throws SQLException {
        if (text.length() != 10 || text.charAt(4) != '-' || text.charAt(7) != '-') throw invalid();
        try {
            LocalDate value = LocalDate.parse(text);
            if (value.getYear() < 1000 || value.getYear() > 9999) throw invalid();
            return value.toString();
        } catch (DateTimeException failure) {
            throw invalid();
        }
    }

    private static String binaryTime(byte[] raw, int scale) throws SQLException {
        if (raw.length == 0) return formatTime(0, scale);
        if (raw.length != 8 && raw.length != 12) throw invalid();
        int negative = unsigned(raw[0]);
        long hours = little(raw, 1, 4) * 24 + unsigned(raw[5]);
        int minutes = unsigned(raw[6]);
        int seconds = unsigned(raw[7]);
        long micros = raw.length == 12 ? little(raw, 8, 4) : 0;
        if (negative > 1 || unsigned(raw[5]) > 23 || hours > 838 || minutes > 59 || seconds > 59 || micros > 999999) throw invalid();
        long value = (hours * 3600 + minutes * 60 + seconds) * 1000000 + micros;
        return formatTime(negative == 1 ? -value : value, scale);
    }

    private static String time(String text, int scale) throws SQLException {
        boolean negative = text.startsWith("-");
        String clock = negative ? text.substring(1) : text;
        int first = clock.indexOf(':');
        if (first < 2 || first > 3 || clock.length() < first + 6 || clock.charAt(first + 3) != ':') throw invalid();
        long hours = digits(clock, 0, first);
        long minutes = digits(clock, first + 1, first + 3);
        long seconds = digits(clock, first + 4, first + 6);
        int end = first + 6;
        long micros = 0;
        if (clock.length() != end) {
            int width = clock.length() - end - 1;
            if (clock.charAt(end) != '.' || width < 1 || width > 6) throw invalid();
            micros = digits(clock, end + 1, clock.length());
            for (int i = width; i < 6; i++) micros *= 10;
        }
        if (hours > 838 || minutes > 59 || seconds > 59) throw invalid();
        long value = (hours * 3600 + minutes * 60 + seconds) * 1000000 + micros;
        return formatTime(negative ? -value : value, scale);
    }

    private static String formatTime(long value, int scale) throws SQLException {
        if (scale < 0 || scale > 6 || value < -MAX_TIME_MICROS || value > MAX_TIME_MICROS) throw invalid();
        long absolute = Math.abs(value);
        long seconds = absolute / 1000000;
        int micros = (int) (absolute % 1000000);
        int quantum = 1;
        for (int i = scale; i < 6; i++) quantum *= 10;
        if (micros % quantum != 0) throw invalid();
        String clock = String.format(Locale.ROOT, "%s%02d:%02d:%02d", value < 0 ? "-" : "", seconds / 3600, seconds / 60 % 60, seconds % 60);
        return scale == 0 ? clock : clock + "." + String.format(Locale.ROOT, "%06d", micros).substring(0, scale);
    }

    private static long digits(String value, int start, int end) throws SQLException {
        long result = 0;
        for (int i = start; i < end; i++) {
            char digit = value.charAt(i);
            if (digit < '0' || digit > '9') throw invalid();
            result = result * 10 + digit - '0';
        }
        return result;
    }

    private static int unsigned(byte value) { return value & 255; }

    private static long little(byte[] value, int offset, int length) {
        long result = 0;
        for (int i = 0; i < length; i++) result |= (long) unsigned(value[offset + i]) << (8 * i);
        return result;
    }

    private static SQLDataException invalid() { return new SQLDataException("Invalid exact MySQL DATE/TIME value", "22007"); }
}
