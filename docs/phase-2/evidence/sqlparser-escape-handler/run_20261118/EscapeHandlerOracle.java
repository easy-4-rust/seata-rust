import java.io.BufferedWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.function.Supplier;
import org.apache.seata.sqlparser.EscapeHandler;
import org.apache.seata.sqlparser.EscapeSymbol;
import org.apache.seata.sqlparser.struct.TableMeta;

public final class EscapeHandlerOracle {
    private static final class Handler implements EscapeHandler {
        final EscapeSymbol symbols;
        final TableMeta table;
        final boolean escape;
        String trace = "";

        Handler(EscapeSymbol symbols, TableMeta table, boolean escape) {
            this.symbols = symbols;
            this.table = table;
            this.escape = escape;
        }

        @Override
        public boolean checkIfKeyWords(String name) {
            return "select".equals(name);
        }

        @Override
        public boolean checkIfNeedEscape(String name, TableMeta metadata) {
            trace += "need:" + hex(name) + ":" + (metadata == table) + ";";
            return escape;
        }

        @Override
        public EscapeSymbol getEscapeSymbol() {
            trace += "symbol;";
            return symbols == null ? EscapeHandler.super.getEscapeSymbol() : symbols;
        }
    }

    private static String hex(String value) {
        if (value == null) {
            return "null";
        }
        StringBuilder result = new StringBuilder();
        for (char unit : value.toCharArray()) {
            result.append(String.format(Locale.ROOT, "%04x", (int) unit));
        }
        return result.toString();
    }

    private static String observe(Supplier<Object> operation, Handler handler, String input) {
        handler.trace = "";
        try {
            Object result = operation.get();
            if (result instanceof Boolean) {
                return result + "\t" + handler.trace;
            }
            return hex((String) result) + "\t" + (result == input) + "\t" + handler.trace;
        } catch (RuntimeException error) {
            return error.getClass().getName() + "\t" + handler.trace;
        }
    }

    public static void main(String[] args) throws Exception {
        try (BufferedWriter out = Files.newBufferedWriter(Path.of(args[0]))) {
            for (int mode = 0; mode < 3; mode++) {
                EscapeSymbol symbols = mode == 0 ? null : mode == 1
                        ? new EscapeSymbol('[', ']') : new EscapeSymbol('.');
                String left = mode == 0 ? "\"" : mode == 1 ? "[" : ".";
                String right = mode == 0 ? "\"" : mode == 1 ? "]" : ".";
                String[] names = {null, "", " ", "\u0000\u0020", "\u2003", "\u00a0", "select",
                        "schema.id", "one.two.three", ".id", "schema.", ".", "\ud800.\udfff",
                        left, right, left + right, left + "s" + right, left + "s" + right + ".id",
                        "schema." + left + "id" + right, left + "s" + right + "." + left + "id" + right,
                        "  " + left + "s" + right + "  ", left + "a" + right + ".b." + left + "c" + right};
                for (boolean escape : new boolean[]{true, false}) {
                    TableMeta table = new TableMeta();
                    Handler handler = new Handler(symbols, table, escape);
                    for (String name : names) {
                        String prefix = mode + "\t" + escape + "\t" + hex(name) + "\t";
                        out.write(prefix + "contains\t" + observe(() -> handler.containsEscape(name), handler, name) + "\n");
                        out.write(prefix + "add\t" + observe(() -> handler.addColNameEscape(name, table), handler, name) + "\n");
                        out.write(prefix + "delete\t" + observe(() -> handler.delColNameEscape(name), handler, name) + "\n");
                    }
                }
            }
            Handler handler = new Handler(null, null, true);
            String single = handler.addColNameEscape("intern-probe");
            String qualified = handler.addColNameEscape("schema.intern-probe");
            out.write("identity\t" + (single == handler.addColNameEscape("intern-probe"))
                    + "\t" + (qualified == handler.addColNameEscape("schema.intern-probe")) + "\n");
        }
    }
}
