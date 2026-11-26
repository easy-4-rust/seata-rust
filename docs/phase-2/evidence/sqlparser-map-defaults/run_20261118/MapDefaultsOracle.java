import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.function.Supplier;
import org.apache.seata.common.util.LowerCaseLinkHashMap;

public final class MapDefaultsOracle {
    private static void record(List<String> rows, String key, Supplier<?> operation) {
        try {
            rows.add(key + "\t" + operation.get());
        } catch (RuntimeException error) {
            rows.add(key + "\t" + error.getClass().getName());
        }
    }

    private record Directional(String name, String matches) {
        @Override public boolean equals(Object other) {
            return other instanceof Directional value && value.name.equals(matches);
        }
        @Override public int hashCode() { return 0; }
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        List<String> out = new ArrayList<>();
        LowerCaseLinkHashMap<Integer> map = new LowerCaseLinkHashMap<>();
        map.put("NullKey", null);
        record(out, "default-null", () -> map.getOrDefault("NULLKEY", 9));
        record(out, "default-missing", () -> map.getOrDefault("missing", 9));
        record(out, "default-null-key", () -> map.getOrDefault(null, 9));
        record(out, "put-null-value", () -> map.putIfAbsent("NULLKEY", 4));
        record(out, "put-existing", () -> map.putIfAbsent("NuLlKeY", 5));
        record(out, "put-origin-folded", () -> map.keySet().contains("nullkey") + ":" + map.get("NullKey"));
        record(out, "replace-value", () -> map.replace("NULLKEY", 6));
        record(out, "replace-missing", () -> map.replace("ABSENT", 7));
        record(out, "replace-wrong", () -> map.replace("NULLKEY", 4, 8));
        record(out, "replace-match", () -> map.replace("NULLKEY", 6, null));
        record(out, "replace-present-null", () -> map.replace("NULLKEY", null, 9));
        record(out, "compute-absent-folded", () -> {
            StringBuilder callback = new StringBuilder();
            Integer result = map.computeIfAbsent("NEW", k -> { callback.append(k); return 2; });
            return callback + ":" + result;
        });
        record(out, "compute-absent-no-callback", () -> map.computeIfAbsent("NEW", k -> { throw new IllegalStateException(); }));
        record(out, "compute-absent-return-null", () -> map.computeIfAbsent("DROP", k -> null) + ":" + map.containsKey("DROP"));
        record(out, "compute-present-remove", () -> {
            StringBuilder callback = new StringBuilder();
            Integer result = map.computeIfPresent("NEW", (k, v) -> { callback.append(k).append(":").append(v); return null; });
            return callback + ":" + result + ":" + map.containsKey("NEW");
        });
        map.put("EMPTY", null);
        record(out, "present-skips-null-value", () -> map.computeIfPresent("EMPTY", (k, v) -> 5) + ":" + map.containsKey("EMPTY"));
        record(out, "compute-removes-null-entry", () -> map.compute("EMPTY", (k, v) -> null) + ":" + map.containsKey("EMPTY"));
        record(out, "compute-rechecks-after-callback", () -> map.compute("SIDE", (k, v) -> {
            map.put(k, 8); return null;
        }) + ":" + map.containsKey("SIDE"));
        record(out, "compute-outputs-own-value", () -> map.compute("SIDE", (k, v) -> {
            map.put(k, 8); return 3;
        }) + ":" + map.get("SIDE"));
        record(out, "compute-null-key-callback-precedence", () -> map.compute(null, null));
        record(out, "compute-null-function", () -> map.compute("SIDE", null));
        record(out, "compute-callback-error", () -> map.compute("SIDE", (k, v) -> { throw new IllegalStateException("test"); }));
        record(out, "compute-after-error", () -> map.get("SIDE"));
        record(out, "compute-locale-callback", () -> {
            StringBuilder callback = new StringBuilder();
            Integer result = map.computeIfAbsent("I", k -> {
                callback.append(k);
                Locale.setDefault(Locale.forLanguageTag("tr"));
                return 7;
            });
            return callback + ":" + result + ":" + map.containsKey("I") + ":" + map.get("i");
        });
        Locale.setDefault(Locale.ENGLISH);
        record(out, "merge-absent", () -> map.merge("MERGE", 11, (old, value) -> { throw new IllegalStateException(); }));
        record(out, "merge-existing", () -> map.merge("MERGE", 4, (old, value) -> old * 10 + value));
        record(out, "merge-remove", () -> map.merge("MERGE", 9, (old, value) -> null) + ":" + map.containsKey("MERGE"));
        record(out, "merge-null-value", () -> map.merge("NEW", null, (old, value) -> 1));
        record(out, "remove-wrong", () -> map.remove("SIDE", 8));
        record(out, "remove-match", () -> map.remove("SIDE", 3));
        record(out, "remove-missing-null", () -> map.remove("ABSENT", null));
        map.put("nullable", null);
        record(out, "remove-present-null", () -> map.remove("NULLABLE", null));
        map.clear();
        map.put("First", 1);
        map.put("Second", 2);
        record(out, "for-each-future-value", () -> {
            List<String> log = new ArrayList<>();
            map.forEach((key, value) -> {
                log.add(key + "=" + value);
                if (key.equals("first")) map.put("SECOND", 8);
            });
            return log;
        });
        record(out, "replace-all-clear-last", () -> {
            map.replaceAll((key, value) -> {
                if (key.equals("second")) map.clear();
                return value * 10;
            });
            return map.size();
        });
        map.put("First", 1);
        map.put("Second", 2);
        record(out, "replace-all-clear-first", () -> {
            map.replaceAll((key, value) -> { map.clear(); return value; });
            return map.size();
        });
        map.put("First", 1);
        record(out, "replace-all-callback-exception", () -> {
            map.replaceAll((key, value) -> { throw new IllegalStateException("callback"); });
            return map.size();
        });
        record(out, "for-each-null-callback", () -> { map.forEach(null); return true; });
        LowerCaseLinkHashMap<Directional> directed = new LowerCaseLinkHashMap<>();
        directed.put("X", new Directional("current", "supplied"));
        record(out, "replace-equals-current-first", () -> directed.replace("X",
                new Directional("supplied", "neither"), new Directional("new", "supplied")));
        record(out, "remove-equals-current-first", () -> directed.remove("X", new Directional("supplied", "neither")));
        LowerCaseLinkHashMap<Object> reentrant = new LowerCaseLinkHashMap<>();
        reentrant.put("Key", new Object() {
            @Override public boolean equals(Object other) {
                reentrant.clear();
                return true;
            }
        });
        record(out, "remove-reenters-public-map-remove", () -> reentrant.remove("KEY", new Object()) + ":" + reentrant.size());
        Files.write(Path.of(args[0]), out);
    }
}
