import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.ByteArrayOutputStream;
import java.io.ByteArrayInputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Timestamp;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.core.compressor.CompressorType;
import org.apache.seata.core.rpc.processor.Pair;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.mysql.MySQLUndoLogManager;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;

/** 固定原MySQL manager写入、取分片及删除方法；Rust已回滚行仅作为逐字节重建的独立夹具。 */
public final class UndoStoreOracle extends MySQLUndoLogManager {
    private static byte[] bytes(Object value) {
        List<?> list=(List<?>)value;byte[] bytes=new byte[list.size()];
        for(int i=0;i<bytes.length;i++)bytes[i]=((Number)list.get(i)).byteValue();return bytes;
    }
    private static void require(boolean value,String reason) {if(!value)throw new AssertionError(reason);}
    private static Connection connect(String[] args) throws SQLException {return DriverManager.getConnection(args[0],args[1],args[2]);}
    private static long count(Connection connection) throws SQLException {
        try(PreparedStatement p=connection.prepareStatement("SELECT COUNT(*) FROM undo_log");ResultSet r=p.executeQuery()){r.next();return r.getLong(1);}
    }
    private Map<String,Object> inspect(Connection connection,String xid,long branchId,byte[] expected) throws Exception {
        byte[] head;String context;int status;boolean sameTime;
        try(PreparedStatement p=connection.prepareStatement("SELECT context,rollback_info,log_status,log_created=log_modified FROM undo_log WHERE xid=? AND branch_id=?")) {
            p.setString(1,xid);p.setLong(2,branchId);
            try(ResultSet r=p.executeQuery()){require(r.next(),"Original root missing");context=r.getString(1);head=r.getBytes(2);status=r.getInt(3);sameTime=r.getBoolean(4);require(!r.next(),"Repeated original root");}
        }
        require(status==0&&sameTime,"Original Normal state/timestamp differs");
        Map<String,String> map=parseContext(context);Pair<Integer,List<byte[]>> tail=getSubRollbackInfo(connection,map.get("subId"),branchId,xid);
        ByteArrayOutputStream all=new ByteArrayOutputStream();all.write(head);for(byte[] part:tail.getSecond())all.write(part);
        require(Arrays.equals(all.toByteArray(),expected),"Original JDBC sub-row retrieval changed native bytes");
        JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();
        BranchUndoLog decoded=parser.decode(all.toByteArray());
        require(Objects.equals(decoded.getXid(),xid)&&decoded.getBranchId()==branchId&&!decoded.getSqlUndoLogs().isEmpty(),"Original parser identity/logs differ");
        Map<String,Object> result=new LinkedHashMap<>();result.put("context",map);result.put("rows",count(connection));result.put("head_bytes",head.length);result.put("tail_bytes",tail.getFirst());result.put("decoded_logs",decoded.getSqlUndoLogs().size());return result;
    }
    public static void main(String[] args) throws Exception {
        ObjectMapper mapper=new ObjectMapper();List<?> cases=mapper.readValue(Files.readAllBytes(Path.of(args[3])),List.class);List<Object> results=new ArrayList<>();
        UndoStoreOracle oracle=new UndoStoreOracle();
        for(Object raw:cases) {
            Map<?,?> input=(Map<?,?>)raw;int packet=((Number)input.get("packet")).intValue();String xid=(String)input.get("xid");long branchId=((Number)input.get("branch_id")).longValue();byte[] content=bytes(input.get("bytes"));
            try(Connection admin=connect(args);PreparedStatement p=admin.prepareStatement("SET GLOBAL max_allowed_packet="+packet)){p.executeUpdate();}
            for(String mode:List.of("original_write","consume_archived_Rust_rows")) {
                try(Connection connection=connect(args);Connection observer=connect(args)) {
                    connection.setAutoCommit(false);
                    try(PreparedStatement p=connection.prepareStatement("SELECT @@session.max_allowed_packet");ResultSet r=p.executeQuery()){r.next();require(r.getInt(1)==packet,"Actual packet setting differs");}
                    if("original_write".equals(mode)) {
                        oracle.insertUndoLogWithNormal(xid,branchId,oracle.buildContext("jackson",CompressorType.NONE,"map",Integer.toString(packet)),content,connection);
                    } else {
                        for(Object item:(List<?>)input.get("rows")) {
                            Map<?,?> row=(Map<?,?>)item;
                            require(Objects.equals(row.get("xid"),xid)&&((Number)row.get("status")).intValue()==0,"Archived Rust row identity/state differs");
                            require(Objects.equals(row.get("log_created"),row.get("log_modified")),"Archived Rust timestamps differ");
                            try(PreparedStatement p=connection.prepareStatement("INSERT INTO undo_log VALUES(?,?,?,?,?,?,?)")) {
                                p.setLong(1,((Number)row.get("branch_id")).longValue());p.setString(2,(String)row.get("xid"));p.setString(3,(String)row.get("context"));p.setBlob(4,new ByteArrayInputStream(bytes(row.get("bytes"))));p.setInt(5,((Number)row.get("status")).intValue());
                                p.setTimestamp(6,Timestamp.valueOf((String)row.get("log_created")));p.setTimestamp(7,Timestamp.valueOf((String)row.get("log_modified")));p.executeUpdate();
                            }
                        }
                    }
                    Map<String,Object> observed=oracle.inspect(connection,xid,branchId,content);require(count(observer)==0,"Original operation implicitly committed Undo");
                    Map<?,?> expectedContext=(Map<?,?>)input.get("root_context");Map<?,?> actual=(Map<?,?>)observed.get("context");
                    for(String key:List.of("serializer","compressorType","map"))require(Objects.equals(actual.get(key),expectedContext.get(key)),"Original context semantic differs");
                    require(((Number)observed.get("rows")).intValue()==((List<?>)input.get("rows")).size(),"Original chunk row count differs");
                    oracle.deleteUndoLog(xid,branchId,connection);require(count(connection)==0,"Original manager did not delete root and all sub-rows");connection.rollback();require(count(observer)==0,"Original rollback left Undo");
                    observed.put("case",input.get("case"));observed.put("mode",mode);observed.put("status","MATCH");results.add(observed);
                }
            }
        }
        try(Connection admin=connect(args);PreparedStatement p=admin.prepareStatement("SET GLOBAL max_allowed_packet=67108864")){p.executeUpdate();}
        Files.write(Path.of(args[4]),mapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(results));System.out.println("source_manager_cases="+results.size()+" MATCH=true commit=false");System.exit(0);
    }
}
