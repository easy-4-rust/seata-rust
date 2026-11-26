import dm.jdbc.driver.DmdbTimestamp;
import java.time.Instant;
import java.time.ZoneOffset;

class DmdbCalendarBoundaryOracle {
    public static void main(String[] args) {
        for (String input : new String[] {"1500-04-01T00:00:00Z", "1582-10-15T00:00:00Z"}) {
            Instant instant = Instant.parse(input);
            var time = DmdbTimestamp.valueOf(instant.atZone(ZoneOffset.UTC));
            System.out.println(input + "\t" + time.toInstant());
        }
    }
}
