import java.time.OffsetDateTime;
import java.time.ZoneOffset;
import java.util.TimeZone;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;

/** 固定 Jackson 的 OffsetDateTime 数值时间戳观测。 */
public final class OffsetDateTimeOracle {
    public static void main(String[] args) throws Exception {
        var mapper = new ObjectMapper().registerModule(new JavaTimeModule());
        mapper.setTimeZone(TimeZone.getTimeZone("UTC"));
        for (String text : new String[]{
                "2025-01-15T10:30:45Z", "1970-01-01T00:00:00Z", "1970-01-01T00:00:00.000000001Z",
                "1969-12-31T23:59:59.999999999Z", "1969-12-31T23:59:58.000000001Z",
                "2025-01-15T10:30:45.123456789+05:30", "1970-01-01T00:00:00.001-00:00:01",
                "-999999999-01-01T00:00:00Z", "+999999999-12-31T23:59:59.999999999Z"}) {
            var source = OffsetDateTime.parse(text);
            String encoded = mapper.writeValueAsString(source);
            var restored = mapper.readValue(encoded, OffsetDateTime.class);
            System.out.printf("TIME\t%s\t%d\t%d\t%d\t%s\t%d\t%d\t%d%n", text,
                    source.toEpochSecond(), source.getNano(), source.getOffset().getTotalSeconds(),
                    encoded, restored.toEpochSecond(), restored.getNano(), restored.getOffset().getTotalSeconds());
        }
        for (String text : new String[]{"0", "1E-9", "-1E-9", "1.1234567899", "-1.1234567899", "0.0000000001", "-0.0000000001"}) {
            var restored = mapper.readValue(text, OffsetDateTime.class);
            System.out.printf("READ\t%s\t%d\t%d%n", text, restored.toEpochSecond(), restored.getNano());
        }
        for (String text : new String[]{"1e-2147483648", "1.0e-2147483647"}) {
            try {
                mapper.readValue(text, OffsetDateTime.class);
                throw new AssertionError("expected invalid decimal metadata");
            } catch (NumberFormatException expected) {
                System.out.printf("REJECT\t%s\t%s%n", text, expected.getClass().getSimpleName());
            }
        }
        var parser = new org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser();
        parser.init();
        var branch = new org.apache.seata.rm.datasource.undo.BranchUndoLog();
        branch.setXid("offset-native:2014");
        branch.setBranchId(42);
        var log = new org.apache.seata.rm.datasource.undo.SQLUndoLog();
        log.setSqlType(org.apache.seata.sqlparser.SQLType.UPDATE);
        var image = new org.apache.seata.rm.datasource.sql.struct.TableRecords();
        image.setTableName("table_records_test");
        var row = new org.apache.seata.rm.datasource.sql.struct.Row();
        row.add(new org.apache.seata.rm.datasource.sql.struct.Field("id", java.sql.Types.INTEGER, 1));
        row.add(new org.apache.seata.rm.datasource.sql.struct.Field(
                "time_col", java.sql.Types.TIMESTAMP_WITH_TIMEZONE, OffsetDateTime.parse("2025-01-15T10:30:45Z")));
        image.getRows().add(row);
        log.setAfterImage(image);
        branch.setSqlUndoLogs(new java.util.ArrayList<>(java.util.List.of(log)));
        byte[] encoded = parser.encode(branch);
        var field = parser.decode(encoded).getSqlUndoLogs().get(0).getAfterImage().getRows().get(0).getFields().get(1);
        System.out.println("UNDO\t" + field.getValue().getClass().getName()
                + "\t" + new String(encoded, java.nio.charset.StandardCharsets.UTF_8));
        System.out.println("LIMITS\t" + OffsetDateTime.MIN.toLocalDateTime().toEpochSecond(ZoneOffset.UTC)
                + "\t" + OffsetDateTime.MAX.toLocalDateTime().toEpochSecond(ZoneOffset.UTC));
    }
}
