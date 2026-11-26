import java.nio.file.Files;
import java.nio.file.Path;
import java.util.AbstractMap;
import org.apache.seata.common.util.LowerCaseLinkHashMap;

public final class MapViewReentryOracle {
    public static void main(String[] args) throws Exception {
        LowerCaseLinkHashMap<Object> map = new LowerCaseLinkHashMap<>();
        map.put("K", new Object());
        Object query = new Object() {
            @Override public boolean equals(Object stored) {
                map.clear();
                return true;
            }
        };
        boolean removed = map.entrySet().remove(new AbstractMap.SimpleEntry<>("k", query));
        Files.writeString(Path.of(args[0]), removed + "\t" + map.size() + "\t" + map.isEmpty() + "\n");
    }
}
