import org.apache.seata.rm.datasource.undo.*;
import org.apache.seata.rm.datasource.undo.parser.ProtostuffUndoLogParser;
import org.apache.seata.rm.datasource.sql.struct.*;
import java.util.*;
class ProbeProtostuffBounds {
    public static void main(String[] args) {
        var codec = new ProtostuffUndoLogParser();
        codec.init();
        for (String arg : args) {
            try {
                codec.decode(HexFormat.of().parseHex(arg));
                System.out.println("accepted");
            } catch (RuntimeException ex) {
                System.out.println("rejected\t" + ex.getClass().getName());
            }
        }
        var field = new Field("n", 8, Double.longBitsToDouble(0x7ff8000000000123L));
        var row = new Row();
        row.add(field);
        var records = new TableRecords();
        records.setRows(List.of(row));
        var log = new SQLUndoLog();
        log.setAfterImage(records);
        var branch = new BranchUndoLog();
        branch.setSqlUndoLogs(List.of(log));
        System.out.println("nan\t" + HexFormat.of().formatHex(codec.encode(branch)));
    }
}
