import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;

class JacksonUtf8Oracle {
    public static void main(String[] args) {
        BranchUndoLog branch = new BranchUndoLog();
        String xid = new String(new int[] {0xB, 0x1F, 0x1F408, 0x85, 0x2028}, 0, 5);
        branch.setXid(xid);
        branch.setBranchId(42);
        JacksonUndoLogParser parser = new JacksonUndoLogParser();
        parser.init();
        byte[] wire = parser.encode(branch);
        if (!parser.decode(wire).getXid().equals(xid)) {
            throw new AssertionError("xid");
        }
        System.out.writeBytes(wire);
    }
}
