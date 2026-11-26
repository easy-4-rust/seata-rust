import java.math.BigDecimal;
import java.sql.Types;
import java.util.List;
import org.apache.seata.rm.datasource.DataCompareUtils;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.undo.AbstractUndoLogManager;
import org.apache.seata.rm.datasource.undo.parser.FastjsonUndoLogParser;
import org.apache.seata.sqlparser.struct.TableMeta;
import org.mockito.Mockito;

/** 实际执行固定原 DataCompareUtils，观察重复主键与复合主键拼接的比较差异。 */
public final class DataCompareIdentityOracle {
    private DataCompareIdentityOracle() {
    }

    /** 输出原 Java 在三种身份输入上的结果和复合键映射规模。 */
    public static void main(String[] args) {
        List<String> keys = List.of("id", "tenant");
        TableMeta table = Mockito.mock(TableMeta.class);
        Mockito.when(table.getPrimaryKeyOnlyName()).thenReturn(keys);
        Row first = row("first", "tenant");
        Row second = row("second", "tenant");
        Row keyA = row("a_b", "c");
        Row keyB = row("a", "b_c");
        emit("right_duplicate", table, List.of(first, second), List.of(first, first));
        emit("left_duplicate", table, List.of(first, first), List.of(first, second));
        emit("both_duplicate", table, List.of(first, first), List.of(first, first));
        emit("colliding_key_reordered", table, List.of(keyA, keyB), List.of(keyB, keyA));
        emit("different_composite_key", table, List.of(keyA), List.of(keyB));
        System.out.println("colliding_mapper_rows\t" + DataCompareUtils.rowListToMap(List.of(keyA, keyB), keys).size());
        for (String input : List.of("0.0000010", "0.0000001", "-0.0000001200", "0.0000000", "12.0000000")) {
            Row decimal = new Row();
            decimal.add(new Field("id", Types.DECIMAL, new BigDecimal(input)));
            var mapped = DataCompareUtils.rowListToMap(List.of(decimal), List.of("id"));
            if (mapped.size() != 1) {
                throw new AssertionError("Expected one decimal row");
            }
            System.out.println("decimal_key_" + input + "\t" + mapped.keySet().iterator().next());
        }
        emitFastjsonDateError();
    }

    private static void emitFastjsonDateError() {
        AbstractUndoLogManager.setCurrentSerializer(FastjsonUndoLogParser.NAME);
        try {
            DataCompareUtils.isFieldEquals(
                    new Field("date", Types.DATE, "   "),
                    new Field("date", Types.DATE, "2024-03-02"));
            throw new AssertionError("Expected no DATE token");
        } catch (ArrayIndexOutOfBoundsException error) {
            System.out.println("space_only_date\t" + error.getClass().getName() + ":" + error.getMessage());
        } finally {
            AbstractUndoLogManager.removeCurrentSerializer();
        }
    }

    private static void emit(String label, TableMeta table, List<Row> before, List<Row> after) {
        System.out.println(label + "\t" + DataCompareUtils.isRowsEquals(table, before, after).getResult());
    }

    private static Row row(String id, String tenant) {
        Row row = new Row();
        row.add(new Field("id", Types.VARCHAR, id));
        row.add(new Field("tenant", Types.VARCHAR, tenant));
        return row;
    }
}
