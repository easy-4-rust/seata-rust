import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Objects;
import org.apache.seata.sqlparser.SQLParsingException;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.SqlSequenceExpr;

/** 固定Java原类的列元数据及无损UTF-16观察；不修改任何原生产类或JUnit断言。 */
public final class ColumnMetaOracle {
    private static final class CallbackObject {
        private final ColumnMeta column;
        private final String kind;
        private CallbackObject(ColumnMeta column, String kind) { this.column=column; this.kind=kind; }
        @Override public boolean equals(Object other) {
            if ("equals".equals(kind)) { column.setOrdinalPosition(7); }
            return other instanceof CallbackObject;
        }
        @Override public int hashCode() {
            if ("hash".equals(kind)) { column.setOrdinalPosition(7); }
            return 11;
        }
        @Override public String toString() {
            if ("text".equals(kind)) {
                column.setOrdinalPosition(7); column.setIsNullAble("callback null");
                column.setIsAutoincrement("YES"); column.setOnUpdate(true); column.setCaseSensitive(true);
            }
            return "callback";
        }
    }
    private static void row(PrintWriter out, String owner, String method, Object... values) {
        List<String> fields = new ArrayList<>();
        for (Object value : values) { fields.add(String.valueOf(value)); }
        out.println("org.apache.seata.sqlparser." + owner + "#" + method + "\t"
            + HexFormat.of().formatHex(String.join("\u001f", fields).getBytes(StandardCharsets.UTF_8)));
    }

    private static String units(String text) {
        if (Objects.isNull(text)) { return "null"; }
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < text.length(); i++) { out.append(String.format("%04x", (int) text.charAt(i))); }
        return out.toString();
    }

    private static void edge(PrintWriter out, String key, Object... values) {
        List<String> fields = new ArrayList<>();
        for (Object value : values) { fields.add(String.valueOf(value)); }
        out.println(key+"\t"+String.join("\t", fields));
    }

    /** 接收源案例观察路径与补充语义观察路径；只写入显式D/E目录。 */
    public static void main(String[] args) throws Exception {
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            ColumnMeta one = new ColumnMeta(); ColumnMeta two = new ColumnMeta();
            boolean initialEquals = one.equals(two); boolean initialHashEquals = one.hashCode() == two.hashCode();
            one.setTableName("table1"); one.setColumnName("column1"); one.setDataType(1);
            ColumnMeta three = new ColumnMeta(); three.setTableName("table1"); three.setColumnName("column1"); three.setDataType(1);
            row(out, "struct.ColumnMetaTest", "testEqualsAndHashCode", initialEquals, initialHashEquals,
                !one.equals(two), one.hashCode() != two.hashCode(), one.equals(three), one.hashCode() == three.hashCode());
            ColumnMeta column = new ColumnMeta(); boolean initial = column.isAutoincrement();
            column.setIsAutoincrement("YES"); boolean yes = column.isAutoincrement();
            column.setIsAutoincrement("NO"); boolean no = column.isAutoincrement();
            row(out, "struct.ColumnMetaTest", "testAutoincrement", initial, yes, no);
            column = new ColumnMeta(); column.setTableCat("tableCat"); row(out, "struct.ColumnMetaTest", "testGetTableCat", column.getTableCat());
            column = new ColumnMeta(); column.setColumnName("columnName"); row(out, "struct.ColumnMetaTest", "testSetGetColumnName", column.getColumnName());
            column = new ColumnMeta(); column.setDataType(2); row(out, "struct.ColumnMetaTest", "testSetGetDataType", column.getDataType());
            column = new ColumnMeta(); column.setDataTypeName("dataTypeName"); row(out, "struct.ColumnMetaTest", "testSetGetDataTypeName", column.getDataTypeName());
            column = new ColumnMeta(); column.setColumnSize(2); row(out, "struct.ColumnMetaTest", "testSetGetColumnSize", column.getColumnSize());
            column = new ColumnMeta(); column.setDecimalDigits(2); row(out, "struct.ColumnMetaTest", "testSetGetDecimalDigits", column.getDecimalDigits());
            column = new ColumnMeta(); column.setNumPrecRadix(10); row(out, "struct.ColumnMetaTest", "testSetGetNumPrecRadix", column.getNumPrecRadix());
            column = new ColumnMeta(); column.setNullAble(5); row(out, "struct.ColumnMetaTest", "testSetGetNullAble", column.getNullAble());
            column = new ColumnMeta(); column.setRemarks("remarks"); row(out, "struct.ColumnMetaTest", "testSetGetRemarks", column.getRemarks());
            column = new ColumnMeta(); column.setColumnDef("columnDef"); row(out, "struct.ColumnMetaTest", "testSetGetColumnDef", column.getColumnDef());
            column = new ColumnMeta(); column.setSqlDataType(1); row(out, "struct.ColumnMetaTest", "testSetGetSqlDataType", column.getSqlDataType());
            column = new ColumnMeta(); column.setSqlDatetimeSub(2); row(out, "struct.ColumnMetaTest", "testSetGetSqlDatetimeSub", column.getSqlDatetimeSub());
            column = new ColumnMeta(); column.setCharOctetLength(255); row(out, "struct.ColumnMetaTest", "testSetGetCharOctetLength", column.getCharOctetLength());
            column = new ColumnMeta(); column.setOrdinalPosition(3); row(out, "struct.ColumnMetaTest", "testSetGetOrdinalPosition", column.getOrdinalPosition());
            column = new ColumnMeta(); column.setOnUpdate(true); row(out, "struct.ColumnMetaTest", "testSetGetIsOnUpdate", column.isOnUpdate());
            column = new ColumnMeta(); column.setCaseSensitive(true); row(out, "struct.ColumnMetaTest", "testSetGetIsCaseSensitive", column.isCaseSensitive());
            if (out.checkError()) { throw new IllegalStateException("Incomplete original case observations"); }
        }
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(Path.of(args[1]), StandardCharsets.UTF_8))) {
            String baseline = "YES";
            for (int position = 0; position < 3; position++) {
                List<String> accepted = new ArrayList<>();
                for (int code = 0; code <= 65535; code++) {
                    char[] candidate = baseline.toCharArray(); candidate[position] = (char) code;
                    if (baseline.equalsIgnoreCase(new String(candidate))) { accepted.add(String.format("%04x", code)); }
                }
                edge(out, "yes-ignore-case-"+position, String.join(",", accepted));
            }
            char[][] vectors = { {}, {0}, {(char)0xd800}, {(char)0xdc00}, {(char)0xd800,(char)0xdc00},
                {'a',(char)0xd800,'b'}, {(char)0xdc00,(char)0xd800} };
            for (int i = 0; i < vectors.length; i++) {
                String text = new String(vectors[i]);
                edge(out, "utf16-"+i, units(text), text.hashCode());
            }
            char[] complete = new char[65536]; for (int i = 0; i < complete.length; i++) { complete[i]=(char)i; }
            edge(out, "all-utf16-units", units(new String(complete)), new String(complete).hashCode());
            String raw = new String(new char[] {(char)0xd800, 0, (char)0xdc00});
            SqlSequenceExpr sequence = new SqlSequenceExpr(raw, null);
            edge(out, "sequence-raw-initial", units(sequence.getSequence()), units(sequence.getFunction()));
            sequence.setFunction(new String(new char[] {(char)0xdc00})); sequence.setSequence(null);
            edge(out, "sequence-raw-mutated", units(sequence.getSequence()), units(sequence.getFunction()));
            SQLParsingException error = new SQLParsingException(raw);
            edge(out, "exception-raw", units(error.getMessage()), units(error.toString()), Objects.isNull(error.getCause()));
            RuntimeException cause = new RuntimeException(raw); error = new SQLParsingException(cause);
            edge(out, "exception-raw-cause", units(error.getMessage()), units(error.toString()), error.getCause() == cause);
            RuntimeException nullText = new RuntimeException() { @Override public String toString() { return null; } };
            error = new SQLParsingException(nullText);
            edge(out, "exception-null-cause-text", units(error.getMessage()), error.getCause() == nullText);
            ColumnMeta column = new ColumnMeta(); edge(out, "column-default", column.hashCode(), units(column.toString()));
            column.setDataType(Integer.MAX_VALUE); column.setColumnSize(Integer.MAX_VALUE); column.setColumnName(raw);
            edge(out, "column-raw-overflow", column.hashCode(), units(column.toString()));
            ColumnMeta left = new ColumnMeta(); ColumnMeta right = new ColumnMeta();
            right.setOrdinalPosition(7);
            left.setCharOctetLength(new CallbackObject(left, "equals"));
            right.setCharOctetLength(new CallbackObject(left, "equals"));
            boolean equal = left.equals(right);
            edge(out, "column-callback-equals", equal, left.getOrdinalPosition());
            column = new ColumnMeta(); column.setCharOctetLength(new CallbackObject(column, "hash"));
            int hash = column.hashCode(); edge(out, "column-callback-hash", hash, column.getOrdinalPosition());
            column = new ColumnMeta(); column.setCharOctetLength(new CallbackObject(column, "text"));
            String text = column.toString(); edge(out, "column-callback-text", units(text), column.getOrdinalPosition());
            if (out.checkError()) { throw new IllegalStateException("Incomplete UTF16 observations"); }
        }
        Files.writeString(Path.of(args[2]), ColumnMeta.class.getName()+"\t"
            +ColumnMeta.class.getProtectionDomain().getCodeSource().getLocation()+"\n", StandardCharsets.UTF_8);
    }
}
