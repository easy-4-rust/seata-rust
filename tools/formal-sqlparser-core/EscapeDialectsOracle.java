import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.function.Supplier;
import org.apache.seata.common.loader.LoadLevel;
import org.apache.seata.sqlparser.EscapeHandler;
import org.apache.seata.sqlparser.EscapeHandlerFactory;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.IndexMeta;
import org.apache.seata.sqlparser.struct.IndexType;
import org.apache.seata.sqlparser.struct.TableMeta;

public final class EscapeDialectsOracle {
    private static final class Observation {
        private final List<String> rows = new ArrayList<>();

        void add(String prefix, Supplier<?> action) {
            try {
                rows.add(prefix + "\t" + action.get());
            } catch (RuntimeException error) {
                rows.add(prefix + "\t" + error.getClass().getName());
            }
        }
    }

    private static final String[] DB_TYPES = {"mysql", "mariadb", "polardb-x", "dm",
            "kingbase", "oceanbase", "oracle", "oscar", "postgresql", "sqlserver"};

    private static String hex(String input) {
        if (input == null) {
            return "null";
        }
        StringBuilder out = new StringBuilder();
        for (char unit : input.toCharArray()) {
            out.append(String.format(Locale.ROOT, "%04x", (int) unit));
        }
        return out.toString();
    }

    private static List<String> keywords(Class<?> type) {
        for (Class<?> nested : type.getDeclaredClasses()) {
            if (nested.isEnum()) {
                return Arrays.stream(nested.getEnumConstants()).map(e -> ((Enum<?>) e).name()).sorted().toList();
            }
        }
        return keywords(type.getSuperclass());
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        Observation data = new Observation();
        List<String> definitions = new ArrayList<>();
        EscapeHandler mysql = EscapeHandlerFactory.getEscapeHandler("mysql");
        for (String db : DB_TYPES) {
            EscapeHandler provider = EscapeHandlerFactory.getEscapeHandler(db);
            String dbKey = db + "\t";
            var level = provider.getClass().getAnnotation(LoadLevel.class);
            definitions.add(String.join("\t", db, provider.getClass().getName(), level.name(),
                    String.valueOf(level.order()), level.scope().name(),
                    String.valueOf(provider.getClass().getProtectionDomain().getCodeSource().getLocation())));
            data.add(dbKey + "identity", () -> (provider == EscapeHandlerFactory.getEscapeHandler(db.toUpperCase(Locale.ROOT)))
                    + "," + (provider == mysql));
            data.add(dbKey + "symbols", () -> (int) provider.getEscapeSymbol().getLeftSymbol()
                    + "," + (int) provider.getEscapeSymbol().getRightSymbol());
            List<String> words = keywords(provider.getClass());
            Locale.setDefault(Locale.ENGLISH);
            for (String word : words) {
                data.add(dbKey + "keyword\t" + word, () -> provider.checkIfKeyWords(word)
                        + "," + provider.checkIfKeyWords(word.toLowerCase(Locale.ROOT)));
            }
            String[] names = {null, "", " ", "\u0000", "\u00a0", "ZZORDINARY", "zzordinary", "Zzordinary",
                    "select", "insert", "INSERT", "inserted", "schema.select", "one.two.select",
                    "one.two.ZzOrdinary", "`select`", "\"select\"", "[select]", "\ud800", "\udfff.A", "É", "ß"};
            for (String language : new String[]{"en", "tr"}) {
                Locale.setDefault(Locale.forLanguageTag(language));
                for (String name : names) {
                    String prefix = dbKey + "name\t" + language + "\t" + hex(name);
                    for (String context : new String[]{"none", "absent", "sensitive", "insensitive"}) {
                        TableMeta table = null;
                        if (!context.equals("none")) {
                            table = new TableMeta();
                            if (!context.equals("absent") && name != null) {
                                ColumnMeta column = new ColumnMeta();
                                column.setCaseSensitive(context.equals("sensitive"));
                                table.getAllColumns().put(name.trim(), column);
                            }
                        }
                        TableMeta selected = table;
                        data.add(prefix + "\t" + context, () -> provider.checkIfNeedEscape(name, selected));
                    }
                    data.add(prefix + "\tadd", () -> hex(provider.addColNameEscape(name)));
                    data.add(prefix + "\tdelete", () -> hex(provider.delColNameEscape(name)));
                }
            }
            Locale.setDefault(Locale.ENGLISH);
            TableMeta table = new TableMeta();
            IndexMeta index = new IndexMeta();
            index.setIndextype(IndexType.PRIMARY);
            List<ColumnMeta> columns = new ArrayList<>();
            for (String name : new String[]{"select", "schema.Mixed", "UNCHANGED", null}) {
                ColumnMeta column = new ColumnMeta();
                column.setColumnName(name);
                columns.add(column);
            }
            index.setValues(columns);
            table.getAllIndexes().put("primary", index);
            data.add(dbKey + "primary-keys", () -> String.join(",", table.getEscapePkNameList(db).stream()
                    .map(EscapeDialectsOracle::hex).toList()));
        }
        Files.write(Path.of(args[0]), data.rows);
        Files.write(Path.of(args[1]), definitions);
    }
}
