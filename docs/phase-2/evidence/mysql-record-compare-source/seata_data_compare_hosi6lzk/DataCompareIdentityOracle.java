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
        emitRowSerializers();
        emitRowConversionFailure();
        emitRowKeyBeforeConversion();
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

    private static void emitRowSerializers() {
        compareRowSerializer("unset", null);
        compareRowSerializer("different_case", "Fastjson");
        compareRowSerializer("fastjson", FastjsonUndoLogParser.NAME);
    }

    private static void compareRowSerializer(String label, String serializer) {
        Field left = new Field("id", Types.BIGINT, 42);
        Field right = new Field("id", Types.BIGINT, 42L);
        if (serializer != null) {
            AbstractUndoLogManager.setCurrentSerializer(serializer);
        }
        try {
            boolean equal = DataCompareUtils.isRowsEquals(
                    idTable(), List.of(fields(left)), List.of(fields(right))).getResult();
            rowObservation("serializer_" + label, equal + ":" + (left.getValue() instanceof Long));
        } finally {
            AbstractUndoLogManager.removeCurrentSerializer();
        }
    }

    private static void emitRowConversionFailure() {
        Field first = new Field("p", Types.BIGINT, 21);
        Field second = new Field("p", Types.BIGINT, 42);
        Field invalid = new Field("a", Types.DATE, "   ");
        List<Row> left = List.of(
                fields(new Field("id", Types.VARCHAR, "duplicate"), first),
                fields(invalid, new Field("id", Types.VARCHAR, "duplicate"), second));
        List<Row> right = List.of(
                fields(new Field("id", Types.VARCHAR, "duplicate")),
                fields(new Field("p", Types.BIGINT, 42L), new Field("id", Types.VARCHAR, "duplicate"),
                        new Field("a", Types.DATE, "2024-03-02")));
        AbstractUndoLogManager.setCurrentSerializer(FastjsonUndoLogParser.NAME);
        try {
            DataCompareUtils.isRowsEquals(idTable(), left, right);
            throw new AssertionError("Expected a DATE error");
        } catch (ArrayIndexOutOfBoundsException error) {
            if (!"   ".equals(invalid.getValue())) {
                throw new AssertionError("Invalid field was changed");
            }
            rowObservation("duplicate_then_date_failure",
                    error.getClass().getName() + ":" + Integer.valueOf(21).equals(first.getValue())
                            + ":" + Long.valueOf(42).equals(second.getValue()));
        } finally {
            AbstractUndoLogManager.removeCurrentSerializer();
        }
    }

    private static void emitRowKeyBeforeConversion() {
        Field left = new Field("id", Types.DATE, "2024-03-02 00:00:00");
        Field right = new Field("id", Types.DATE, "2024-03-02");
        AbstractUndoLogManager.setCurrentSerializer(FastjsonUndoLogParser.NAME);
        try {
            boolean equal = DataCompareUtils.isRowsEquals(
                    idTable(), List.of(fields(left)), List.of(fields(right))).getResult();
            boolean unchanged = "2024-03-02 00:00:00".equals(left.getValue())
                    && "2024-03-02".equals(right.getValue());
            rowObservation("keys_before_conversion", equal + ":" + unchanged);
        } finally {
            AbstractUndoLogManager.removeCurrentSerializer();
        }
    }

    private static TableMeta idTable() {
        TableMeta table = Mockito.mock(TableMeta.class);
        Mockito.when(table.getPrimaryKeyOnlyName()).thenReturn(List.of("id"));
        return table;
    }

    private static Row fields(Field... fields) {
        Row row = new Row();
        for (Field field : fields) {
            row.add(field);
        }
        return row;
    }

    private static void rowObservation(String name, String value) {
        System.out.println("SEATA_COMPARE_ROWS\t" + name + "\t" + value);
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
