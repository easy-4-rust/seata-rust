import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.function.Supplier;
import org.apache.seata.sqlparser.EscapeHandlerFactory;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.IndexMeta;
import org.apache.seata.sqlparser.struct.IndexType;
import org.apache.seata.sqlparser.struct.TableMeta;
import org.apache.seata.sqlparser.util.ColumnUtils;

public final class EscapePkOracle {
    private static void record(List<String> rows, String key, Supplier<?> action) {
        try {
            rows.add(key + "\t" + action.get());
        } catch (RuntimeException error) {
            rows.add(key + "\t" + error.getClass().getName());
        }
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        List<String> rows = new ArrayList<>();
        var handler = EscapeHandlerFactory.getEscapeHandler("mysql");
        record(rows, "same-provider", () -> handler == EscapeHandlerFactory.getEscapeHandler("MYSQL"));
        record(rows, "unknown", () -> EscapeHandlerFactory.getEscapeHandler("invalid"));
        record(rows, "empty-db", () -> EscapeHandlerFactory.getEscapeHandler(""));
        record(rows, "null-db", () -> EscapeHandlerFactory.getEscapeHandler(null));
        record(rows, "untrimmed-db", () -> EscapeHandlerFactory.getEscapeHandler(" mysql "));
        for (String word : Files.readAllLines(Path.of(args[1]))) {
            record(rows, "kw/" + word, () -> handler.checkIfKeyWords(word) + "," +
                    handler.checkIfKeyWords(word.toLowerCase(Locale.ROOT)));
        }
        record(rows, "add-blank", () -> ColumnUtils.addEscape(" ", "invalid"));
        record(rows, "delete-blank", () -> ColumnUtils.delEscape(" ", "invalid"));
        record(rows, "add-empty-list", () -> {
            List<String> empty = new ArrayList<>();
            return empty == ColumnUtils.addEscape(empty, "invalid");
        });
        record(rows, "add-null-list", () -> ColumnUtils.addEscape((List<String>) null, "invalid"));
        record(rows, "add-list", () -> ColumnUtils.addEscape(Arrays.asList("select", "sch.order", null, "id", "`select`"), "mysql"));
        record(rows, "delete-list", () -> ColumnUtils.delEscape(Arrays.asList("`sch`.`order`", null, "id"), "mysql"));
        record(rows, "framing", () -> ColumnUtils.addEscape("one.two.order", "mysql"));
        record(rows, "nbsp", () -> ColumnUtils.addEscape("\u00a0select", "mysql"));
        record(rows, "locale-en", () -> ColumnUtils.addEscape("insert", "mysql"));
        Locale.setDefault(Locale.forLanguageTag("tr"));
        record(rows, "locale-tr", () -> ColumnUtils.addEscape("insert", "mysql"));
        Locale.setDefault(Locale.ENGLISH);
        TableMeta table = new TableMeta();
        table.setTableName("events");
        record(rows, "missing-pk-before-missing-provider", () -> table.getEscapePkNameList("invalid"));
        IndexMeta index = new IndexMeta();
        index.setIndextype(IndexType.PRIMARY);
        List<ColumnMeta> columns = new ArrayList<>();
        for (String name : Arrays.asList("select", "schema.order", null, "id")) {
            ColumnMeta column = new ColumnMeta();
            column.setColumnName(name);
            columns.add(column);
        }
        index.setValues(columns);
        table.getAllIndexes().put("primary", index);
        record(rows, "table-entrypoint", () -> table.getEscapePkNameList("mysql"));
        Files.write(Path.of(args[0]), rows);
    }
}
