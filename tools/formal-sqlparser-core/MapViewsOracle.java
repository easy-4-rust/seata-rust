import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.function.Supplier;
import org.apache.seata.common.util.LowerCaseLinkHashMap;

public final class MapViewsOracle {
    private static void observe(List<String> rows, String key, Supplier<?> operation) {
        try {
            rows.add(key + "\t" + operation.get());
        } catch (RuntimeException error) {
            rows.add(key + "\t" + error.getClass().getName());
        }
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        List<String> rows = new ArrayList<>();
        LowerCaseLinkHashMap<Integer> map = new LowerCaseLinkHashMap<>();
        map.put("First", 1);
        map.put("SECOND", 2);
        var entries = map.entrySet();
        var values = map.values();
        var keys = map.keySet();
        var cursor = entries.iterator();
        observe(rows, "remove-before-next", () -> { cursor.remove(); return true; });
        Map.Entry<String, Integer> first = cursor.next();
        observe(rows, "first", () -> first.getKey() + ":" + first.getValue());
        observe(rows, "set-live", () -> first.setValue(3));
        observe(rows, "values-live", values::toString);
        observe(rows, "put-replace", () -> map.put("FIRSt", 4));
        observe(rows, "handle-saw-put", first::getValue);
        cursor.remove();
        observe(rows, "removed", () -> map.containsKey("first"));
        observe(rows, "removed-entry-retained", first::getValue);
        observe(rows, "double-remove", () -> { cursor.remove(); return true; });
        observe(rows, "second-after-remove", () -> {
            var next = cursor.next();
            return next.getKey() + ":" + next.getValue();
        });
        observe(rows, "exhausted-next", cursor::next);
        observe(rows, "set-detached", () -> first.setValue(5));
        map.put("FIRST", 6);
        observe(rows, "detached-vs-reinserted", () -> first.getValue() + ":" + map.get("FIRST"));
        observe(rows, "old-key-snapshot", () -> keys.contains("First") + ":" + keys.contains("FIRST"));
        observe(rows, "current-key-snapshot", () -> map.keySet().contains("FIRST") + ":" + map.keySet().contains("First"));
        var valuesCursor = values.iterator();
        observe(rows, "values-first", valuesCursor::next);
        valuesCursor.remove();
        observe(rows, "values-remove-target-only", () -> map.containsKey("second") + ":" + map.keySet().contains("SECOND"));
        var current = entries.iterator().next();
        entries.clear();
        observe(rows, "entries-clear-target-only", () -> entries.size() + ":" + map.keySet().size());
        observe(rows, "clear-detached-entry", () -> current.getKey() + ":" + current.getValue());
        map.put("THIRD", null);
        observe(rows, "values-visible-after-clear", values::toString);
        var changed = entries.iterator();
        changed.next();
        map.put("fourth", 8);
        observe(rows, "changed-remove", () -> { changed.remove(); return true; });
        observe(rows, "changed-exhausted-next", changed::next);
        observe(rows, "values-remove-null", () -> values.remove(null));
        observe(rows, "null-key-origin-retained", () -> map.keySet().contains("THIRD"));
        observe(rows, "entries-remove-lowercase-only", () ->
                entries.remove(Map.entry("FOURTH", 8)) + ":" + entries.remove(Map.entry("fourth", 8)));
        observe(rows, "entries-remove-origin-retained", () -> map.keySet().contains("fourth"));
        map.put("values-Clear", 9);
        values.clear();
        observe(rows, "values-clear-target-only", () -> values.size() + ":" + map.keySet().contains("values-Clear"));
        map.clear();
        observe(rows, "map-clear-both", () -> values.size() + ":" + map.keySet().size());

        for (int count : new int[]{1, 11, 12, 13, 21, 48, 65}) {
            map.clear();
            for (int i=0; i<count; i++) map.put("Key-" + (31 * i), i);
            observe(rows, "snapshot-order/" + count, () -> String.join(",", map.keySet()));
        }
        Files.write(Path.of(args[0]), rows);
    }
}
