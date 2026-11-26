import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Objects;
import org.apache.seata.sqlparser.EscapeSymbol;
import org.apache.seata.sqlparser.SQLParsingException;
import org.apache.seata.sqlparser.SQLType;
import org.apache.seata.sqlparser.SqlParserType;
import org.apache.seata.sqlparser.struct.IndexType;
import org.apache.seata.sqlparser.struct.NotPlaceholderExpr;
import org.apache.seata.sqlparser.struct.Null;
import org.apache.seata.sqlparser.struct.SqlDefaultExpr;
import org.apache.seata.sqlparser.struct.SqlMethodExpr;
import org.apache.seata.sqlparser.struct.SqlSequenceExpr;
import org.apache.seata.sqlparser.util.JdbcConstants;

/** 用固定原Java生产类计算原23案例的全部可观察结果，不替代原JUnit执行。 */
public final class CoreValueOracle {
    private static void row(PrintWriter out, String owner, String method, Object... values) {
        List<String> fields = new ArrayList<>();
        for (Object value : values) { fields.add(String.valueOf(value)); }
        out.println("org.apache.seata.sqlparser." + owner + "#" + method + "\t"
            + HexFormat.of().formatHex(String.join("\u001f", fields).getBytes(StandardCharsets.UTF_8)));
    }

    /** 参数为观察文件、完整常量文件和加载来源文件；所有输出仅写入自有D盘目录。 */
    public static void main(String[] args) throws Exception {
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            row(out, "EscapeSymbolTest", "testGetLeftSymbol", (int) new EscapeSymbol('"', '"').getLeftSymbol());
            row(out, "EscapeSymbolTest", "testGetRightSymbol", (int) new EscapeSymbol('"', '"').getRightSymbol());
            row(out, "SQLTypeTest", "testValue", SQLType.SELECT.value(), SQLType.INSERT.value());
            row(out, "SQLTypeTest", "testValueOf", SQLType.valueOf(0), SQLType.valueOf(1));
            try { SQLType.valueOf(100); throw new AssertionError("Invalid SQLType accepted"); }
            catch (IllegalArgumentException error) {
                row(out, "SQLTypeTest", "testValueOfInvalid", error.getClass().getName(), error.getMessage());
            }
            row(out, "struct.IndexTypeTest", "testValue", IndexType.PRIMARY.value(), IndexType.NORMAL.value(),
                IndexType.UNIQUE.value(), IndexType.FULL_TEXT.value());
            row(out, "struct.IndexTypeTest", "testValueOf", IndexType.valueOf(0), IndexType.valueOf(1),
                IndexType.valueOf(2), IndexType.valueOf(3));
            try { IndexType.valueOf(4); throw new AssertionError("Invalid IndexType accepted"); }
            catch (IllegalArgumentException error) {
                row(out, "struct.IndexTypeTest", "testInvalidValueOf", error.getClass().getName(), error.getMessage());
            }
            SQLParsingException message = new SQLParsingException("Test message");
            row(out, "SQLParsingExceptionTest", "testConstructorWithMessage", message.getMessage(), Objects.isNull(message.getCause()));
            Throwable cause = new IllegalArgumentException("Test cause");
            SQLParsingException both = new SQLParsingException("Test message", cause);
            row(out, "SQLParsingExceptionTest", "testConstructorWithMessageAndCause", both.getMessage(), Objects.equals(cause, both.getCause()));
            SQLParsingException causeOnly = new SQLParsingException(cause);
            row(out, "SQLParsingExceptionTest", "testConstructorWithCause", causeOnly.getMessage(), Objects.equals(cause, causeOnly.getCause()));
            SqlSequenceExpr expr = new SqlSequenceExpr();
            row(out, "struct.SqlSequenceExprTest", "testDefaultConstructor", Objects.isNull(expr.getSequence()), Objects.isNull(expr.getFunction()));
            expr = new SqlSequenceExpr("mySequence", "myFunction");
            row(out, "struct.SqlSequenceExprTest", "testParameterizedConstructor", expr.getSequence(), expr.getFunction());
            expr = new SqlSequenceExpr(); expr.setSequence("newSequence");
            row(out, "struct.SqlSequenceExprTest", "testSetGetSequence", expr.getSequence());
            expr = new SqlSequenceExpr(); expr.setFunction("newFunction");
            row(out, "struct.SqlSequenceExprTest", "testSetGetFunction", expr.getFunction());
            row(out, "struct.SqlMethodExprTest", "testGet", SqlMethodExpr.get().equals(SqlMethodExpr.get()));
            row(out, "struct.SqlMethodExprTest", "testToString", SqlMethodExpr.get().toString().trim());
            row(out, "struct.SqlDefaultExprTest", "testGet", SqlDefaultExpr.get().equals(SqlDefaultExpr.get()));
            row(out, "struct.SqlDefaultExprTest", "testToString", SqlDefaultExpr.get().toString().trim());
            row(out, "struct.NullTest", "testGet", Null.get().equals(Null.get()));
            row(out, "struct.NullTest", "testToString", Null.get().toString().trim());
            row(out, "struct.NotPlaceHolderExprTest", "testGet", NotPlaceholderExpr.get().equals(NotPlaceholderExpr.get()));
            row(out, "struct.NotPlaceHolderExprTest", "testToString", NotPlaceholderExpr.get().toString());
            if (out.checkError()) { throw new IllegalStateException("Incomplete original observation"); }
        }
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(Path.of(args[1]), StandardCharsets.UTF_8))) {
            for (SQLType value : SQLType.values()) { out.println("SQLType\t" + value.name() + "\t" + value.value()); }
            for (IndexType value : IndexType.values()) { out.println("IndexType\t" + value.name() + "\t" + value.value()); }
            for (Field field : JdbcConstants.class.getFields()) { out.println("JdbcConstants\t" + field.getName() + "\t" + field.get(null)); }
            out.println("SqlParserType\tSQL_PARSER_TYPE_DRUID\t" + SqlParserType.SQL_PARSER_TYPE_DRUID);
            out.println("SqlParserType\tSQL_PARSER_TYPE_ANTLR\t" + SqlParserType.SQL_PARSER_TYPE_ANTLR);
            if (out.checkError()) { throw new IllegalStateException("Incomplete original constants"); }
        }
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(Path.of(args[2]), StandardCharsets.UTF_8))) {
            for (Class<?> type : List.of(EscapeSymbol.class, SQLType.class, SQLParsingException.class,
                    SqlParserType.class, JdbcConstants.class, IndexType.class, SqlSequenceExpr.class,
                    SqlMethodExpr.class, SqlDefaultExpr.class, Null.class, NotPlaceholderExpr.class)) {
                out.println(type.getName() + "\t" + type.getProtectionDomain().getCodeSource().getLocation());
            }
        }
    }
}
