import org.apache.seata.rm.datasource.undo.*;
import org.apache.seata.rm.datasource.undo.parser.KryoUndoLogParser;
import org.apache.seata.rm.datasource.sql.struct.*;
import org.apache.seata.sqlparser.SQLType;
import java.util.*;
class ProbeKryoPrimitives {
    public static void main(String[] args) {
        var codec = new KryoUndoLogParser();
        codec.init();
        for (KeyType key : KeyType.values()) {
            for (SQLType kind : new SQLType[] {null, SQLType.UPDATE}) {
                for (Object value : new Object[] {12345, -2049L, "txt", null, new java.sql.Timestamp(1), true, -0.5d}) {
                    var field = new Field("ID", 4, value);
                    field.setKeyType(key);
                    var row = new Row();
                    row.add(field);
                    var records = new TableRecords();
                    records.setRows(new ArrayList<>(List.of(row)));
                    records.setTableName("tab");
                    var log = new SQLUndoLog();
                    log.setSqlType(kind);
                    log.setTableName("tab");
                    log.setAfterImage(records);
                    var branch = new BranchUndoLog();
                    branch.setBranchId(123456);
                    branch.setSqlUndoLogs(new ArrayList<>(List.of(log)));
                    String hex = HexFormat.of().formatHex(codec.encode(branch));
                    String prefix = "7374727563742e4669656ce4";
                    System.out.printf("%s\t%s\t%s\t%s\t%s%n", key, kind, value == null ? "null" : value.getClass().getSimpleName(),
                        hex.substring(hex.indexOf(prefix) + prefix.length()), hex);
                }
            }
        }
    }
}
