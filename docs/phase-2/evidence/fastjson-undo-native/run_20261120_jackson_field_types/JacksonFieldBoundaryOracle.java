import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import javax.sql.rowset.serial.SerialBlob;
import java.nio.charset.StandardCharsets;
import java.sql.Timestamp;
import java.time.LocalDateTime;
import java.time.ZoneOffset;

class JacksonFieldBoundaryOracle {
    public static void main(String[] args) throws Exception {
        var parser = new JacksonUndoLogParser();
        parser.init();
        JacksonUndoLogParser.setZoneOffset(ZoneOffset.UTC);
        var property = JacksonUndoLogParser.class.getDeclaredField("mapper");
        property.setAccessible(true);
        var mapper = (ObjectMapper)property.get(parser);
        String template = "{\"@class\":\"org.apache.seata.rm.datasource.sql.struct.Field\",\"name\":\"time\",\"keyType\":\"NULL\",\"type\":93,\"value\":[\"%s\",[-1123,777222333]]}";
        var local = (LocalDateTime)mapper.readValue(template.formatted("java.time.LocalDateTime"), Field.class).getValue();
        var timestamp = (Timestamp)mapper.readValue(template.formatted("java.sql.Timestamp"), Field.class).getValue();
        System.out.printf("SEATA_JACKSON_BOUNDARY\tlocal\t%d\t%d%n", local.toEpochSecond(ZoneOffset.UTC), local.getNano());
        System.out.printf("SEATA_JACKSON_BOUNDARY\ttimestamp\t%d\t%d%n", timestamp.toInstant().getEpochSecond(), timestamp.getNanos());
        byte[] blob = mapper.writeValueAsBytes(new Field("empty_blob", 2004, new SerialBlob(new byte[0])));
        System.out.printf("SEATA_JACKSON_BOUNDARY\tempty-blob\t%s%n", new String(blob, StandardCharsets.UTF_8));
        byte[] exponent = mapper.writeValueAsBytes(new Field("large_double", 8, 10000000.0));
        System.out.printf("SEATA_JACKSON_BOUNDARY\tdouble\t%s%n", new String(exponent, StandardCharsets.UTF_8));
    }
}
