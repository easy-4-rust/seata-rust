import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Properties;
import org.apache.seata.rm.datasource.LockKeyOracle;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.sql.struct.cache.MysqlTableMetaCache;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.apache.seata.sqlparser.SQLType;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 自有数据库相同SQL/值的原JDBC和锁键方法对照，不模拟完整Java AT或TC。 */
public final class LockNativeOracle extends MysqlTableMetaCache {
    public static void main(String[] args) throws Exception {
        UndoPreparedOracle.main(args);
        Path output=Path.of(args[1]);String profile=args[2];
        JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();
        BranchUndoLog branch=parser.decode(Files.readAllBytes(output.resolve(profile+".json")));
        Properties properties=new Properties();properties.setProperty("user","root");properties.setProperty("password",Objects.requireNonNull(System.getenv("SEATA_MYSQL_PASSWORD")));
        properties.setProperty("useSSL","false");properties.setProperty("allowPublicKeyRetrieval","true");properties.setProperty("useInformationSchema","true");properties.setProperty("characterEncoding","UTF-8");properties.setProperty("serverTimezone","UTC");
        if("numeric_tinyint".equals(profile))properties.setProperty("tinyInt1isBit","false");
        String url="jdbc:mysql://127.0.0.1:"+System.getenv("SEATA_MYSQL_PORT")+"/"+System.getenv("SEATA_MYSQL_DATABASE");
        List<String> keys=new ArrayList<>();LockNativeOracle oracle=new LockNativeOracle();
        try(Connection connection=DriverManager.getConnection(url,properties)) {
            connection.setAutoCommit(false);
            for(SQLUndoLog log:branch.getSqlUndoLogs()) {
                TableRecords records=log.getSqlType()==SQLType.DELETE?log.getBeforeImage():log.getAfterImage();
                records.setTableMeta(oracle.fetchSchema(connection,records.getTableName()));keys.add(LockKeyOracle.key(records));
            }
            for(String sql:new String[]{"INSERT INTO lock_pk_reverse(a,b) VALUES(1,2)","INSERT INTO lock_tiny_keys(id) VALUES(0),(1)",
                "INSERT INTO lock_decimal_keys(id) VALUES(0.00000000000000000000),(0.00000010000000000000)"}) {
                try(Statement statement=connection.createStatement()) {statement.executeUpdate(sql);}
                String table=sql.substring("INSERT INTO ".length(),sql.indexOf('('));
                TableMeta meta=oracle.fetchSchema(connection,table);
                try(Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT * FROM "+table+" ORDER BY "+("lock_pk_reverse".equals(table)?"b,a":"id"))) {keys.add(LockKeyOracle.key(TableRecords.buildRecords(meta,rows)));}
            }
            try(PreparedStatement insert=connection.prepareStatement("INSERT INTO lock_binary_keys(id) VALUES(?),(?)")) {insert.setBytes(1,new byte[]{0,-1});insert.setBytes(2,new byte[]{-128,127});insert.executeUpdate();}
            TableMeta meta=oracle.fetchSchema(connection,"lock_binary_keys");
            try(Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT * FROM lock_binary_keys ORDER BY id")) {keys.add(LockKeyOracle.key(TableRecords.buildRecords(meta,rows)));}
            try(Statement statement=connection.createStatement()) {statement.executeUpdate(Files.readString(Path.of(args[3])));}
            meta=oracle.fetchSchema(connection,"lock_hash_collision");
            try(Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT * FROM lock_hash_collision")) {keys.add(LockKeyOracle.key(TableRecords.buildRecords(meta,rows)));}
            connection.rollback();
            for(String table:List.of("lock_pk_reverse","lock_tiny_keys","lock_decimal_keys","lock_binary_keys","lock_hash_collision")) {
                try(Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT COUNT(*) FROM "+table)) {rows.next();if(rows.getLong(1)!=0)throw new AssertionError("Original lock fixture rollback");}
            }
        }
        if(keys.size()!=9)throw new AssertionError("Original actual lock log count");
        String full=LockKeyOracle.combined(keys);List<String> set=Objects.isNull(full)?List.of():Arrays.stream(full.split(";")).sorted().toList();
        Map<String,Object> result=Map.of("keys",keys,"full_key_set",set);
        ObjectMapper mapper=new ObjectMapper();Path candidate=Path.of(Objects.requireNonNull(System.getenv("SEATA_MYSQL_LOCK_KEYS_OUT"))).resolve(profile+".json");
        Object expected=mapper.readValue(mapper.writeValueAsBytes(result),Object.class);Object actual=mapper.readValue(Files.readAllBytes(candidate),Object.class);
        if(!Objects.equals(expected,actual))throw new AssertionError("Original actual JDBC/Rust lock key differential failed: "+profile);
        Path pairs=output.resolve("codec-pairs-"+profile+".json");
        Files.write(pairs,mapper.writeValueAsBytes(List.of(Map.of("case",profile,"reference",output.resolve(profile+".json").toString(),
            "candidate",Path.of(Objects.requireNonNull(System.getenv("SEATA_MYSQL_CODEC_OUT"))).resolve(profile+".json").toString()))));
        UndoDocumentPeer.main(new String[]{"compare",output.toString(),output.resolve("codec-compare-"+profile).toString(),pairs.toString()});
        Files.write(output.resolve("locks-"+profile+".json"),mapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(result));
        System.out.println("original_actual_lock_key_MATCH profile="+profile+" logs=9 rollback=true");System.exit(0);
    }
}
