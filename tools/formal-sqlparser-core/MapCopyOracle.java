import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.function.BiConsumer;
import java.util.function.Supplier;
import org.apache.seata.common.util.LowerCaseLinkHashMap;

public final class MapCopyOracle {
    private static final class CloneableMap<V> extends LowerCaseLinkHashMap<V> {
        LowerCaseLinkHashMap<V> copy() throws CloneNotSupportedException { return super.clone(); }
    }

    private static final class ChangingSource extends LinkedHashMap<String, Integer> {
        int calls;
        final boolean fail;

        ChangingSource(boolean fail) { this.fail = fail; }

        @Override public void forEach(BiConsumer<? super String, ? super Integer> visitor) {
            if (++calls == 1) {
                visitor.accept("I", 1);
                visitor.accept("FirstOnly", 6);
            } else {
                Locale.setDefault(Locale.forLanguageTag("tr"));
                visitor.accept("I", 9);
                if (fail) throw new IllegalStateException("second pass");
                visitor.accept("SecondOnly", 3);
            }
        }
    }

    private static void observe(List<String> rows, String key, Supplier<?> operation) {
        try {
            rows.add(key + "\t" + operation.get());
        } catch (RuntimeException error) {
            rows.add(key + "\t" + error.getClass().getName());
        }
    }

    private static boolean put(LowerCaseLinkHashMap<Integer> target, LinkedHashMap<String,Integer> source) {
        target.putAll(source);
        return true;
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        List<String> rows = new ArrayList<>();
        LowerCaseLinkHashMap<Integer> map = new LowerCaseLinkHashMap<>();
        map.put("existing", 7);
        LinkedHashMap<String,Integer> source = new LinkedHashMap<>();
        source.put("A", 1);
        source.put(null, 2);
        source.put("B", 3);
        observe(rows, "null-key-first-pass", () -> put(map, source));
        observe(rows, "null-key-origins", () -> map.keySet().contains("A") + ":" + map.keySet().contains("B"));
        observe(rows, "null-key-target", map::toString);
        observe(rows, "null-source", () -> put(map, null));

        map.clear();
        source.clear();
        source.put("Alpha", 1);
        source.put("ALPHA", 8);
        source.put("Nullable", null);
        map.putAll(source);
        observe(rows, "bulk-order-and-update", map::toString);
        observe(rows, "bulk-keeps-last-origin", () -> map.keySet().contains("ALPHA") + ":" + map.keySet().contains("Alpha"));
        var cursor = map.entrySet().iterator();
        cursor.next();
        map.putAll(map);
        observe(rows, "self-alias", map::toString);
        observe(rows, "self-alias-rewrites-origin", () -> map.keySet().contains("alpha") + ":" + map.keySet().contains("ALPHA"));
        observe(rows, "self-alias-cursor-valid", () -> cursor.next().getKey());

        map.clear();
        ChangingSource changing = new ChangingSource(false);
        map.putAll(changing);
        observe(rows, "two-visits", () -> changing.calls);
        observe(rows, "two-visits-target", map::toString);
        observe(rows, "two-visits-origins", () -> map.keySet().contains("FirstOnly") + ":" + map.keySet().contains("SecondOnly"));
        observe(rows, "turkish-target", () -> map.containsKey("I") + ":" + map.containsKey("i") + ":" + map.get("I"));
        Locale.setDefault(Locale.ENGLISH);
        observe(rows, "english-target", () -> map.containsKey("I") + ":" + map.containsKey("\u0131"));

        Locale.setDefault(Locale.ENGLISH);
        map.clear();
        ChangingSource failing = new ChangingSource(true);
        observe(rows, "second-pass-error", () -> put(map, failing));
        observe(rows, "second-pass-visited", () -> failing.calls);
        observe(rows, "second-pass-keeps-origin-and-target", () -> map.keySet().size() + ":" + map);
        Locale.setDefault(Locale.ENGLISH);

        CloneableMap<StringBuilder> original = new CloneableMap<>();
        StringBuilder shared = new StringBuilder("a");
        original.put("CamelCase", shared);
        original.put("RemovedOrigin", new StringBuilder("stale"));
        original.entrySet().removeIf(entry -> entry.getKey().equals("removedorigin"));
        original.put("Nullable", null);
        LowerCaseLinkHashMap<StringBuilder> copy = original.copy();
        observe(rows, "copy-rebuilds-origins", () -> copy.keySet().contains("camelcase") + ":" +
                copy.keySet().contains("CamelCase") + ":" + copy.keySet().contains("RemovedOrigin"));
        observe(rows, "copy-shares-value", () -> copy.get("camelcase") == shared);
        observe(rows, "copy-shares-no-entry", () -> copy.entrySet().iterator().next() == original.entrySet().iterator().next());
        shared.append("b");
        observe(rows, "copy-sees-mutable-value", () -> copy.get("camelcase"));
        copy.put("CAMELCASE", new StringBuilder("copy"));
        observe(rows, "copy-structure-independent", () -> original.get("camelcase") + ":" + copy.get("camelcase"));
        observe(rows, "copy-null-entry", () -> copy.containsKey("Nullable") + ":" + copy.get("Nullable"));
        observe(rows, "copy-constructor", () -> new LowerCaseLinkHashMap<>(original));
        observe(rows, "copy-null-source", () -> new LowerCaseLinkHashMap<Integer>(null));
        Files.write(Path.of(args[0]), rows);
    }
}
