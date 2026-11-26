import org.apache.seata.rm.datasource.undo.*;
import org.apache.seata.rm.datasource.undo.parser.ProtostuffUndoLogParser;
import org.apache.seata.rm.datasource.sql.struct.*;
import java.util.*;
class ProbeProtostuff {
    public static void main(String[] args) {
        var parser = new ProtostuffUndoLogParser();
        parser.init();
        for (Object value : new Object[] {
            null, -17, -2049L, true, false, new byte[] {1, 2}, -0.5d, new java.math.BigDecimal("7.250"),
            new java.math.BigInteger("18446744073709551615"),
        }) {
            var field = new Field("n", -3, value);
            var row = new Row();
            row.add(field);
            var records = new TableRecords();
            records.setRows(List.of(row));
            var log = new SQLUndoLog();
            log.setAfterImage(records);
            var branch = new BranchUndoLog();
            branch.setBranchId(-1);
            branch.setSqlUndoLogs(List.of(log));
            byte[] wire = parser.encode(branch);
            System.out.println((value == null ? "null" : value.getClass().getSimpleName()) + "\t" + HexFormat.of().formatHex(wire));
        }
        System.out.println("empty\t" + HexFormat.of().formatHex(parser.encode(new BranchUndoLog())));
    }
}
