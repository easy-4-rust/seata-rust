import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.Statement;
import java.sql.Types;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.Properties;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.sql.struct.cache.MysqlTableMetaCache;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.apache.seata.sqlparser.SQLType;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 相同固定SQL/参数的独立原JDBC取行/parser对照；不模拟完整Java AT/TC流程。 */
public final class UndoPreparedOracle extends MysqlTableMetaCache {
    public static void main(String[] args) throws Exception {
        Path sqlRoot=Path.of(args[0]);Path out=Path.of(args[1]);Files.createDirectories(out);
        String profile=args[2];Class.forName("com.mysql.cj.jdbc.Driver");
        Properties props=new Properties();props.setProperty("user","root");props.setProperty("password",Objects.requireNonNull(System.getenv("SEATA_MYSQL_PASSWORD")));
        props.setProperty("useSSL","false");props.setProperty("allowPublicKeyRetrieval","true");props.setProperty("useInformationSchema","true");
        props.setProperty("characterEncoding","UTF-8");props.setProperty("serverTimezone","UTC");
        if("numeric_tinyint".equals(profile))props.setProperty("tinyInt1isBit","false");
        String url="jdbc:mysql://127.0.0.1:"+System.getenv("SEATA_MYSQL_PORT")+"/"+System.getenv("SEATA_MYSQL_DATABASE");
        try(Connection connection=DriverManager.getConnection(url,props)) {
            try(Statement statement=connection.createStatement()) {statement.execute("SET NAMES utf8mb4");statement.execute("SET SESSION sql_mode='STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION,NO_BACKSLASH_ESCAPES',transaction_isolation='REPEATABLE-READ'");}
            connection.setAutoCommit(false);UndoPreparedOracle oracle=new UndoPreparedOracle();
            TableMeta nativeMeta=oracle.fetchSchema(connection,"mysql_native_identity");List<SQLUndoLog> logs=new ArrayList<>();
            byte[][] binary={new byte[]{-1},new byte[]{0,-1},new byte[]{0,-1},new byte[]{-128,-1},new byte[]{0,-1,127,-128},new byte[]{0,1,-1}};
            try(PreparedStatement insert=connection.prepareStatement(Files.readString(sqlRoot.resolve("native_insert.sql")))) {
                for(int i=0;i<binary.length;i++)insert.setBytes(i+1,binary[i]);if(insert.executeUpdate()!=1)throw new AssertionError("native INSERT count");
            }
            logs.add(log(SQLType.INSERT,nativeMeta,TableRecords.empty(nativeMeta),records(connection,nativeMeta,"SELECT * FROM mysql_native_identity ORDER BY id")));
            TableRecords before=records(connection,nativeMeta,"SELECT * FROM mysql_native_identity ORDER BY id");
            try(Statement statement=connection.createStatement()) {if(statement.executeUpdate(Files.readString(sqlRoot.resolve("native_update.sql")))!=1)throw new AssertionError("native UPDATE count");}
            logs.add(log(SQLType.UPDATE,nativeMeta,before,records(connection,nativeMeta,"SELECT * FROM mysql_native_identity ORDER BY id")));
            before=records(connection,nativeMeta,"SELECT * FROM mysql_native_identity ORDER BY id");
            try(Statement statement=connection.createStatement()) {if(statement.executeUpdate(Files.readString(sqlRoot.resolve("native_delete.sql")))!=1)throw new AssertionError("native DELETE count");}
            logs.add(log(SQLType.DELETE,nativeMeta,before,TableRecords.empty(nativeMeta)));
            TableMeta composite=oracle.fetchSchema(connection,"execution_records");
            try(PreparedStatement insert=connection.prepareStatement(Files.readString(sqlRoot.resolve("composite_insert.sql")))) {
                // 原8.0.27 setObject(BigInteger)会按BIGINT.longValue推断；显式精确DECIMAL绑定保留同一完整数值。
                insert.setLong(1,Long.MIN_VALUE);insert.setBigDecimal(2,new BigDecimal(new BigInteger("18446744073709551615")));
                insert.setBigDecimal(3,new BigDecimal("12345678901234567890123456789012345.123456789012345678901234567890"));insert.setString(4,"中文🦀");insert.setBytes(5,new byte[]{0,-1});
                insert.setLong(6,2);insert.setBigDecimal(7,new BigDecimal(new BigInteger("7")));insert.setBigDecimal(8,new BigDecimal("-0.000000000000000000000000000001"));insert.setNull(9,Types.VARCHAR);insert.setNull(10,Types.VARBINARY);
                if(insert.executeUpdate()!=2)throw new AssertionError("composite INSERT count");
            }
            logs.add(log(SQLType.INSERT,composite,TableRecords.empty(composite),records(connection,composite,"SELECT * FROM execution_records ORDER BY a,b")));
            BranchUndoLog branch=new BranchUndoLog();branch.setXid("codec-fixture:xid");branch.setBranchId(42);branch.setSqlUndoLogs(logs);
            JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();Files.write(out.resolve(profile+".json"),parser.encode(branch));
            connection.rollback();
            try(Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT COUNT(*) FROM execution_records")) {rows.next();if(rows.getLong(1)!=0)throw new AssertionError("reference rollback");}
            System.out.println("original_JDBC_prepared_logs=4 profile="+profile+" rollback=true");
        }
    }
    private static SQLUndoLog log(SQLType type,TableMeta meta,TableRecords before,TableRecords after) {
        SQLUndoLog log=new SQLUndoLog();log.setSqlType(type);log.setTableName(meta.getTableName());log.setBeforeImage(before);log.setAfterImage(after);return log;
    }
    private static TableRecords records(Connection connection,TableMeta meta,String sql) throws Exception {
        try(Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery(sql)) {return TableRecords.buildRecords(meta,rows);}
    }
}
