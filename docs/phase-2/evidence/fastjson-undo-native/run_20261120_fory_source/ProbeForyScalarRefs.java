import org.apache.seata.rm.datasource.undo.*;
import org.apache.seata.rm.datasource.undo.parser.ForyUndoLogParser;
import org.apache.seata.rm.datasource.sql.struct.*;
import java.util.*;
class ProbeForyScalarRefs {
    public static void main(String[] args) {
        var codec = new ForyUndoLogParser();
        codec.init();
        for (Object value : new Object[] {Integer.valueOf(7), Long.valueOf(7), Boolean.TRUE, "same", Double.valueOf(-0.5)}) {
            var first = new Row();
            first.add(new Field("v", 4, value));
            var second = new Row();
            second.add(new Field("v", 4, value));
            var records = new TableRecords();
            records.setRows(new ArrayList<>(List.of(first, second)));
            var log = new SQLUndoLog();
            log.setAfterImage(records);
            var branch = new BranchUndoLog();
            branch.setSqlUndoLogs(new ArrayList<>(List.of(log)));
            byte[] bytes = codec.encode(branch);
            var decoded = codec.decode(bytes).getSqlUndoLogs().get(0).getAfterImage().getRows();
            if (decoded.get(0).getFields().get(0).getValue() != decoded.get(1).getFields().get(0).getValue()) {
                throw new AssertionError("object reference");
            }
            System.out.printf("scalar\t%s\t%s%n", value.getClass().getSimpleName(), HexFormat.of().formatHex(bytes));
        }
    }
}
