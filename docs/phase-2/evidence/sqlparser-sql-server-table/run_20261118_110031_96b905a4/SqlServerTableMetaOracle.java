import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.SqlServerTableMeta;
import org.apache.seata.sqlparser.struct.TableMeta;

/** Observes the original subclass and inherited dispatch on original Seata classes. */
public final class SqlServerTableMetaOracle {
    public static void main(String[] args) throws Exception {
        TableMeta base = new TableMeta();
        SqlServerTableMeta sqlServer = new SqlServerTableMeta();
        SqlServerTableMeta peer = new SqlServerTableMeta();
        boolean initial = sqlServer.isTableIdentifyExistence();
        int initialHash = sqlServer.hashCode();
        sqlServer.setTableIdentifyExistence(true);
        boolean enabled = sqlServer.isTableIdentifyExistence();
        boolean peerFlag = peer.isTableIdentifyExistence();
        String equality = base.equals(sqlServer) + "\t" + sqlServer.equals(base)
                + "\t" + sqlServer.equals(peer) + "\t" + peer.equals(sqlServer)
                + "\t" + sqlServer.equals(null) + "\t" + sqlServer.equals("other");
        String hashes = initialHash + "\t" + sqlServer.hashCode() + "\t" + base.hashCode();
        TableMeta view = sqlServer;
        view.setTableName("events");
        ColumnMeta column = new ColumnMeta();
        sqlServer.getAllColumns().put("ID", column);
        String inherited = sqlServer.getTableName() + "\t" + (view.getColumnMeta("id") == column)
                + "\t" + sqlServer.equals(base) + "\t" + base.equals(sqlServer)
                + "\t" + view.equals(sqlServer);
        sqlServer.setTableIdentifyExistence(false);
        Files.write(Path.of(args[0]), List.of(
                "identity\t" + initial + "\t" + enabled + "\t" + peerFlag + "\t" + sqlServer.isTableIdentifyExistence(),
                "equals\t" + equality, "hash\t" + hashes, "inherited\t" + inherited));
    }
}
