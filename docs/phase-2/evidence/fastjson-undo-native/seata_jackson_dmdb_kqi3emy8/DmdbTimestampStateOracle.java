import com.fasterxml.jackson.databind.ObjectMapper;
import dm.jdbc.driver.DmdbTimestamp;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.Arrays;
import java.util.Date;

class DmdbTimestampStateOracle {
    static String state(DmdbTimestamp time) {
        return time.toInstant().getEpochSecond() + "\t" + time.toInstant().getNano() + "\t"
            + time.dtype + "\t" + time.scale + "\t" + time.getTimezone() + "\t" + Arrays.toString(time.getDt());
    }

    static void inspect(ObjectMapper mapper, String label, DmdbTimestamp original) throws Exception {
        byte[] encoded = mapper.writeValueAsBytes(new Field(label, 93, original));
        DmdbTimestamp restored = (DmdbTimestamp)mapper.readValue(encoded, Field.class).getValue();
        System.out.printf("DMDB_STATE\t%s\toriginal\t%s%n", label, state(original));
        System.out.printf("DMDB_STATE\t%s\trestored\t%s%n", label, state(restored));
        System.out.printf("DMDB_EQUAL\t%s\t%b\t%b%n", label, java.util.Objects.deepEquals(original, restored),
            original.toInstant().equals(restored.toInstant()));
    }

    public static void main(String[] args) throws Exception {
        var parser = new JacksonUndoLogParser();
        parser.init();
        JacksonUndoLogParser.setZoneOffset(ZoneOffset.UTC);
        var property = JacksonUndoLogParser.class.getDeclaredField("mapper");
        property.setAccessible(true);
        var mapper = (ObjectMapper)property.get(parser);
        Instant source = Instant.ofEpochMilli(1721985847000L);
        inspect(mapper, "zoned", DmdbTimestamp.valueOf(source.atZone(ZoneOffset.UTC)));
        inspect(mapper, "unzoned", DmdbTimestamp.valueOf(new Date(source.toEpochMilli())));
        inspect(mapper, "nanos", DmdbTimestamp.valueOf(source.plusNanos(12345).atZone(ZoneOffset.UTC)));
        inspect(mapper, "later", DmdbTimestamp.valueOf(new Date(source.toEpochMilli() + 1)));
        inspect(mapper, "east", DmdbTimestamp.valueOf(source.atZone(ZoneOffset.ofHours(8))));
    }
}
