import com.fasterxml.jackson.databind.ObjectMapper;
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
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.core.compressor.CompressorType;
import org.apache.seata.rm.datasource.undo.mysql.MySQLUndoLogManager;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;

/** 固定原GlobalFinished写入与Normal冲突；独立控制提交，不实现完整undo入口或补偿。 */
public final class UndoFenceOracle extends MySQLUndoLogManager {
    private static void require(boolean value,String message){if(!value)throw new AssertionError(message);}
    private static Connection connect(String[] args)throws SQLException{return DriverManager.getConnection(args[0],args[1],args[2]);}
    private static byte[] bytes(Object raw){List<?> list=(List<?>)raw;byte[] result=new byte[list.size()];for(int i=0;i<result.length;i++)result[i]=((Number)list.get(i)).byteValue();return result;}
    private static long count(Connection c)throws SQLException{try(PreparedStatement p=c.prepareStatement("SELECT COUNT(*) FROM undo_log");ResultSet r=p.executeQuery()){r.next();return r.getLong(1);}}
    private Map<String,Object> inspect(Connection c,String xid,long id)throws Exception{
        try(PreparedStatement p=c.prepareStatement("SELECT branch_id,xid,context,rollback_info,log_status,log_created,log_modified FROM undo_log WHERE xid=? AND branch_id=?")){
            p.setString(1,xid);p.setLong(2,id);
            try(ResultSet r=p.executeQuery()){
                require(r.next(),"Original guard missing");Map<String,Object> row=new LinkedHashMap<>();
                row.put("branch_id",r.getLong(1));row.put("xid",r.getString(2));row.put("context",parseContext(r.getString(3)));row.put("bytes_hex",HexFormat.of().formatHex(r.getBytes(4)));row.put("status",r.getInt(5));row.put("created",r.getTimestamp(6).toString());row.put("modified",r.getTimestamp(7).toString());
                require(!r.next(),"Repeated original identity");require(Objects.equals(row.get("xid"),xid)&&Objects.equals(row.get("branch_id"),id),"Actual original identity changed");
                require(Objects.equals(row.get("status"),1)&&Objects.equals(row.get("bytes_hex"),"7b7d")&&Objects.equals(row.get("created"),row.get("modified")),"Actual original guard changed");return row;
            }
        }
    }
    public static void main(String[] args)throws Exception{
        ObjectMapper mapper=new ObjectMapper();Map<?,?> input=mapper.readValue(Files.readAllBytes(Path.of(args[3])),Map.class);Map<?,?> row=(Map<?,?>)input.get("row");String xid=(String)row.get("xid");long id=((Number)row.get("branch_id")).longValue();
        UndoFenceOracle oracle=new UndoFenceOracle();JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();List<Object> result=new ArrayList<>();
        require(Objects.equals(xid,"fence:xid")&&id==42&&((Number)row.get("status")).intValue()==1&&Arrays.equals(bytes(row.get("bytes")),parser.getDefaultContent()),"Archived Rust identity/guard differs");
        for(String mode:List.of("original_write","consume_archived_Rust_guard")){
            try(Connection admin=connect(args);PreparedStatement p=admin.prepareStatement("DELETE FROM undo_log")){p.executeUpdate();}
            try(Connection c=connect(args);Connection observer=connect(args)){
                c.setAutoCommit(false);
                if(Objects.equals(mode,"original_write")){oracle.insertUndoLogWithGlobalFinished(xid,id,parser,c);}
                else{
                    try(PreparedStatement p=c.prepareStatement("INSERT INTO undo_log VALUES(?,?,?,?,?,?,?)")){
                        p.setLong(1,id);p.setString(2,(String)row.get("xid"));p.setString(3,(String)row.get("context"));p.setBlob(4,new ByteArrayInputStream(bytes(row.get("bytes"))));p.setInt(5,((Number)row.get("status")).intValue());p.setTimestamp(6,Timestamp.valueOf((String)row.get("log_created")));p.setTimestamp(7,Timestamp.valueOf((String)row.get("log_modified")));p.executeUpdate();
                    }
                }
                require(count(observer)==0,"Guard write implicitly committed");c.commit();Map<String,Object> before=oracle.inspect(observer,xid,id);
                require(Objects.equals(before.get("context"),oracle.parseContext((String)row.get("context"))),"Original guard context semantics differ");
                if(Objects.equals(mode,"consume_archived_Rust_guard")){
                    require(Objects.equals(before.get("created"),Timestamp.valueOf((String)row.get("log_created")).toString())&&Objects.equals(before.get("modified"),Timestamp.valueOf((String)row.get("log_modified")).toString()),"Archived Rust timestamp repaired");
                }
                int conflict=0;
                try{oracle.insertUndoLogWithNormal(xid,id,oracle.buildContext("jackson",CompressorType.NONE,"map","67108864"),bytes(input.get("normal_bytes")),c);}
                catch(SQLException error){conflict=error.getErrorCode();}
                require(conflict==1062,"Original late Normal not rejected by unique identity");c.rollback();require(before.equals(oracle.inspect(observer,xid,id)),"Late Normal changed original guard");
                before.put("mode",mode);before.put("status_result","MATCH");before.put("normal_error",conflict);result.add(before);
            }
        }
        Files.write(Path.of(args[4]),mapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(result));System.out.println("original_guard_cases=2 MATCH=true full_undo=false");System.exit(0);
    }
}
