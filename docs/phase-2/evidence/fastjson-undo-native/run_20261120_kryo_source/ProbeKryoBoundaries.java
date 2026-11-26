import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.parser.KryoUndoLogParser;
import java.util.HexFormat;
class ProbeKryoBoundaries {
    public static void main(String[] args) {
        var codec = new KryoUndoLogParser();
        codec.init();
        for (String xid : new String[] {null,"","x","ID","é","中","🐈","x".repeat(32),"x".repeat(33),"x".repeat(70)}) {
            var branch = new BranchUndoLog();
            branch.setXid(xid);
            byte[] bytes = codec.encode(branch);
            System.out.printf("text\t%s\t%s%n", xid, HexFormat.of().formatHex(bytes));
        }
        for (long id : new long[] {Long.MIN_VALUE,Long.MAX_VALUE,-1L,0L,1L}) {
            var branch = new BranchUndoLog();
            branch.setBranchId(id);
            byte[] bytes = codec.encode(branch);
            System.out.printf("id\t%d\t%s%n", id, HexFormat.of().formatHex(bytes));
        }
    }
}
