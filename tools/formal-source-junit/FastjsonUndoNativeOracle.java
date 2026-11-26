import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Types;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Objects;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.KeyType;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.apache.seata.rm.datasource.undo.UndoLogParser;
import org.apache.seata.rm.datasource.undo.parser.Fastjson2UndoLogParser;
import org.apache.seata.rm.datasource.undo.parser.FastjsonUndoLogParser;
import org.apache.seata.sqlparser.SQLType;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 使用固定 Java Fastjson 或 Fastjson2 Undo 编解码器，记录原载荷及解码后的物理值。 */
public final class FastjsonUndoNativeOracle {
    public static void main(String[] args) throws Exception {
        UndoLogParser parser;
        boolean wide = args.length == 2 && "fastjson2-wide".equals(args[1]);
        if (wide || (args.length == 2 && "fastjson2".equals(args[1]))) {
            var fastjson2 = new Fastjson2UndoLogParser();
            fastjson2.init();
            parser = fastjson2;
        } else {
            var fastjson = new FastjsonUndoLogParser();
            fastjson.init();
            parser = fastjson;
        }
        var branch = new BranchUndoLog();
        branch.setXid("fastjson-native:42");
        branch.setBranchId(42L);
        branch.setSqlUndoLogs(new ArrayList<>(List.of(
                log(SQLType.INSERT, empty(), records("insert", wide)),
                log(SQLType.UPDATE, records("insert", wide), records("update", wide)),
                log(SQLType.DELETE, records("update", wide), empty()))));
        byte[] bytes = parser.encode(branch);
        Files.write(Path.of(args[0]), bytes);
        BranchUndoLog decoded = parser.decode(bytes);
        if (!branch.getXid().equals(decoded.getXid()) || decoded.getBranchId() != 42L) {
            throw new AssertionError("Branch identity changed");
        }
        if (decoded.getSqlUndoLogs().size() != branch.getSqlUndoLogs().size()) {
            throw new AssertionError("SQL log count changed");
        }
        for (int i = 0; i < decoded.getSqlUndoLogs().size(); i++) {
            var log = decoded.getSqlUndoLogs().get(i);
            if (args.length == 2) {
                var original = branch.getSqlUndoLogs().get(i);
                verifyValueClasses(original.getBeforeImage(), log.getBeforeImage());
                verifyValueClasses(original.getAfterImage(), log.getAfterImage());
            }
            System.out.println("LOG\t" + log.getSqlType() + "\t" + log.getTableName());
            observe(log.getBeforeImage());
            observe(log.getAfterImage());
        }
    }

    private static void verifyValueClasses(TableRecords original, TableRecords decoded) {
        if (original.getRows().size() != decoded.getRows().size()) {
            throw new AssertionError("Row count changed");
        }
        for (int row = 0; row < original.getRows().size(); row++) {
            var expected = original.getRows().get(row).getFields();
            var actual = decoded.getRows().get(row).getFields();
            if (expected.size() != actual.size()) {
                throw new AssertionError("Field count changed");
            }
            for (int i = 0; i < expected.size(); i++) {
                Object before = expected.get(i).getValue();
                Object after = actual.get(i).getValue();
                if (Objects.isNull(before) ? Objects.nonNull(after)
                        : Objects.isNull(after) || before.getClass() != after.getClass()) {
                    throw new AssertionError("Field value class changed: " + expected.get(i).getName());
                }
            }
        }
    }

    private static SQLUndoLog log(SQLType type, TableRecords before, TableRecords after) {
        var log = new SQLUndoLog();
        log.setSqlType(type);
        log.setTableName("undo_fixture");
        log.setBeforeImage(before);
        log.setAfterImage(after);
        return log;
    }

    private static TableRecords empty() {
        var meta = new TableMeta();
        meta.setTableName("undo_fixture");
        return TableRecords.empty(meta);
    }

    private static TableRecords records(String text, boolean wide) {
        var records = new TableRecords();
        records.setTableName("undo_fixture");
        var row = new Row();
        var id = new Field("id", Types.BIGINT, 3L);
        id.setKeyType(KeyType.PRIMARY_KEY);
        row.add(id);
        row.add(new Field("small", Types.INTEGER, -7));
        row.add(new Field("wide", Types.BIGINT, Long.MIN_VALUE));
        row.add(new Field("unsigned", Types.BIGINT, new BigInteger(wide ? "1" : "18446744073709551615")));
        row.add(new Field("decimal", Types.DECIMAL, new BigDecimal(wide
                ? "-12345678901234567890123456789012345.678901234567890123456789012300"
                : "-0.0000001200")));
        row.add(new Field("whole", Types.DECIMAL, new BigDecimal("42")));
        row.add(new Field("text", Types.VARCHAR, text + "-螃蟹"));
        row.add(new Field("bytes", Types.VARBINARY, new byte[]{0, -1, 127}));
        row.add(new Field("nullable", Types.VARCHAR, null));
        records.getRows().add(row);
        return records;
    }

    private static void observe(TableRecords image) {
        System.out.println("IMAGE\t" + image.getRows().size());
        for (Row row : image.getRows()) {
            for (Field field : row.getFields()) {
                Object value = field.getValue();
                String canonical;
                if (Objects.isNull(value)) {
                    canonical = "null";
                } else if (value instanceof byte[] bytes) {
                    canonical = HexFormat.of().formatHex(bytes);
                } else if (value instanceof String text) {
                    canonical = HexFormat.of().formatHex(text.getBytes(StandardCharsets.UTF_8));
                } else if (value instanceof BigDecimal decimal) {
                    canonical = decimal.toPlainString();
                } else {
                    canonical = value.toString();
                }
                System.out.printf("FIELD\t%s\t%d\t%s\t%s%n", field.getName(), field.getType(),
                        field.getKeyType(), canonical);
            }
        }
    }
}
