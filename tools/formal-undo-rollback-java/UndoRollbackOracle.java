import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.ByteArrayInputStream;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import java.sql.Timestamp;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.rm.datasource.ConnectionProxy;
import org.apache.seata.rm.datasource.DataCompareUtils;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.sql.struct.cache.MysqlTableMetaCache;
import org.apache.seata.rm.datasource.undo.AbstractUndoExecutor;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.apache.seata.rm.datasource.undo.mysql.MySQLUndoDeleteExecutor;
import org.apache.seata.rm.datasource.undo.mysql.MySQLUndoInsertExecutor;
import org.apache.seata.rm.datasource.undo.mysql.MySQLUndoLogManager;
import org.apache.seata.rm.datasource.undo.mysql.MySQLUndoUpdateExecutor;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 在自有JDBC库上实际调用未修改原执行器、数据校验及日志清理；不启动TC或声称full undo入口。 */
public final class UndoRollbackOracle extends MysqlTableMetaCache {
    private static void require(boolean value,String message){if(!value)throw new AssertionError(message);}
    private static byte[] bytes(Object raw){List<?> list=(List<?>)raw;byte[] value=new byte[list.size()];for(int i=0;i<value.length;i++)value[i]=((Number)list.get(i)).byteValue();return value;}
    private static Connection connect(String[] args,String profile)throws SQLException{return DriverManager.getConnection(args[0]+(Objects.equals(profile,"numeric_tinyint")?"&tinyInt1isBit=false":""),args[1],args[2]);}
    private static final class DialectConnection extends ConnectionProxy {
        DialectConnection(Connection connection){super(null,connection);}
        @Override public String getDbType(){return "mysql";}
    }
    private static final class SourceManager extends MySQLUndoLogManager {
        static void enter(){setCurrentSerializer("jackson");}
        static void exit(){removeCurrentSerializer();}
        static void clean(Connection c)throws SQLException{new SourceManager().deleteUndoLog("rollback:xid",42,c);}
    }
    private static Connection tracked(Connection actual,List<Map<String,Object>> events){
        return (Connection)Proxy.newProxyInstance(UndoRollbackOracle.class.getClassLoader(),new Class<?>[]{Connection.class},(proxy,method,args)->{
            try {
                Object result=method.invoke(actual,args);
                if(Objects.equals(method.getName(),"prepareStatement")&&Objects.nonNull(args)&&args[0] instanceof String){
                    String sql=(String)args[0];PreparedStatement target=(PreparedStatement)result;
                    return Proxy.newProxyInstance(UndoRollbackOracle.class.getClassLoader(),new Class<?>[]{PreparedStatement.class},(p,m,a)->{
                        try {Object value=m.invoke(target,a);if(Objects.equals(m.getName(),"executeUpdate")){Map<String,Object> event=new LinkedHashMap<>();event.put("sql",sql);event.put("affected",value);events.add(event);}return value;}
                        catch(InvocationTargetException error){throw error.getCause();}
                    });
                }
                return result;
            } catch(InvocationTargetException error){throw error.getCause();}
        });
    }
    private static void execute(Connection c,String sql)throws SQLException{try(Statement s=c.createStatement()){s.execute(sql);}}
    private static List<List<Object>> snapshot(Connection c,String table)throws Exception{
        List<List<Object>> rows=new ArrayList<>();try(Statement p=c.createStatement();ResultSet r=p.executeQuery("SELECT * FROM `"+table+"`")){
            while(r.next()) {List<Object> values=new ArrayList<>();for(int i=1;i<=r.getMetaData().getColumnCount();i++){Object value=r.getObject(i);values.add(value instanceof byte[]?"binary:"+HexFormat.of().formatHex((byte[])value):Objects.toString(value,"<null>"));}rows.add(values);}
        }
        rows.sort((a,b)->a.toString().compareTo(b.toString()));return rows;
    }
    private static Map<String,Object> business(Connection c)throws Exception{
        Map<String,Object> rows=new LinkedHashMap<>();for(String table:List.of("undo_work","undo_business")){
            try(ResultSet r=c.getMetaData().getTables(c.getCatalog(),null,table,new String[]{"TABLE"})){if(r.next())rows.put(table,snapshot(c,table));}
        }return rows;
    }
    private static List<Map<String,Object>> undoSnapshot(Connection c)throws SQLException {
        List<Map<String,Object>> rows=new ArrayList<>();try(Statement p=c.createStatement();ResultSet r=p.executeQuery("SELECT xid,branch_id,context,rollback_info,log_status,log_created,log_modified FROM undo_log ORDER BY xid,branch_id")){
            while(r.next()){Map<String,Object> row=new LinkedHashMap<>();row.put("xid",r.getString(1));row.put("branch_id",r.getLong(2));row.put("context",r.getString(3));row.put("bytes_hex",HexFormat.of().formatHex(r.getBytes(4)));row.put("status",r.getInt(5));row.put("created",r.getTimestamp(6).toString());row.put("modified",r.getTimestamp(7).toString());rows.add(row);}
        }return rows;
    }
    private static void insertRows(Connection c,List<?> rows)throws Exception{
        for(Object item:rows){Map<?,?> row=(Map<?,?>)item;require(Objects.equals(row.get("xid"),"rollback:xid")&&((Number)row.get("status")).intValue()==0,"Archived native row identity/state changed");
            try(PreparedStatement p=c.prepareStatement("INSERT INTO undo_log VALUES(?,?,?,?,?,?,?)")){
                p.setLong(1,((Number)row.get("branch_id")).longValue());p.setString(2,(String)row.get("xid"));p.setString(3,(String)row.get("context"));p.setBlob(4,new ByteArrayInputStream(bytes(row.get("bytes"))));p.setInt(5,((Number)row.get("status")).intValue());p.setTimestamp(6,Timestamp.valueOf((String)row.get("created")));p.setTimestamp(7,Timestamp.valueOf((String)row.get("modified")));p.executeUpdate();
            }
            try(PreparedStatement p=c.prepareStatement("SELECT xid,branch_id,context,rollback_info,log_status,log_created,log_modified FROM undo_log WHERE xid=? AND branch_id=?")){
                p.setString(1,(String)row.get("xid"));p.setLong(2,((Number)row.get("branch_id")).longValue());try(ResultSet r=p.executeQuery()){
                    require(r.next()&&Objects.equals(r.getString(1),row.get("xid"))&&r.getLong(2)==((Number)row.get("branch_id")).longValue()&&Objects.equals(r.getString(3),row.get("context"))&&Arrays.equals(r.getBytes(4),bytes(row.get("bytes")))&&r.getInt(5)==0&&Objects.equals(r.getTimestamp(6),Timestamp.valueOf((String)row.get("created")))&&Objects.equals(r.getTimestamp(7),Timestamp.valueOf((String)row.get("modified")))&&!r.next(),"Original native seven-column fixture repaired or changed");
                }
            }
        }
    }
    private static byte[] payload(Map<?,?> input)throws Exception{
        List<?> rows=(List<?>)input.get("rows");Map<?,?> root=null;for(Object item:rows){Map<?,?> row=(Map<?,?>)item;if(((Number)row.get("branch_id")).longValue()==42)root=row;}
        require(Objects.nonNull(root),"Original native root missing");java.io.ByteArrayOutputStream out=new java.io.ByteArrayOutputStream();out.write(bytes(root.get("bytes")));
        String context=(String)root.get("context");for(String entry:context.split("&"))if(entry.startsWith("subId="))for(String text:entry.substring(6).split(",")){
            long id=Long.parseLong(text);Map<?,?> child=null;for(Object item:rows){Map<?,?> row=(Map<?,?>)item;if(((Number)row.get("branch_id")).longValue()==id)child=row;}
            require(Objects.nonNull(child)&&Objects.equals(child.get("context"),"branchId=42"),"Original native child missing");out.write(bytes(child.get("bytes")));
        }return out.toByteArray();
    }
    private static void dirty(Connection c,SQLUndoLog log)throws Exception{
        TableRecords records=log.getAfterImage().getRows().isEmpty()?log.getBeforeImage():log.getAfterImage();Row row=records.getRows().get(0);List<Field> fields=row.getFields();Field chosen=fields.stream().filter(f->!Objects.equals(f.getKeyType().name(),"PRIMARY_KEY")).findFirst().orElseThrow();
        Object changed=chosen.getValue() instanceof Boolean?!((Boolean)chosen.getValue()):chosen.getValue() instanceof Number?(((Number)chosen.getValue()).longValue()==0?1:0):"ORACLE_DIRTY";
        if(log.getAfterImage().getRows().isEmpty()){
            String columns=String.join(",",fields.stream().map(f->"`"+f.getName()+"`").toList());String placeholders=String.join(",",Collections.nCopies(fields.size(),"?"));
            try(PreparedStatement p=c.prepareStatement("INSERT INTO `"+log.getTableName()+"` ("+columns+") VALUES ("+placeholders+")")){for(int i=0;i<fields.size();i++)p.setObject(i+1,fields.get(i)==chosen?changed:fields.get(i).getValue());p.executeUpdate();}
        }else{
            List<Field> keys=fields.stream().filter(f->Objects.equals(f.getKeyType().name(),"PRIMARY_KEY")).toList();String where=String.join(" AND ",keys.stream().map(f->"`"+f.getName()+"`=?").toList());
            try(PreparedStatement p=c.prepareStatement("UPDATE `"+log.getTableName()+"` SET `"+chosen.getName()+"`=? WHERE "+where)){p.setObject(1,changed);for(int i=0;i<keys.size();i++)p.setObject(i+2,keys.get(i).getValue());p.executeUpdate();}
        }
    }
    public static void main(String[] args)throws Exception{
        ObjectMapper mapper=new ObjectMapper();List<?> cases=mapper.readValue(Files.readAllBytes(Path.of(args[3])),List.class);List<Object> results=new ArrayList<>();UndoRollbackOracle oracle=new UndoRollbackOracle();String control=args.length>6?args[6]:"none";
        require(AbstractUndoExecutor.IS_UNDO_DATA_VALIDATION_ENABLE,"Original data validation disabled");
        for(Object item:cases){Map<?,?> input=(Map<?,?>)item;String profile=(String)input.get("profile");JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();BranchUndoLog prototype=parser.decode(payload(input));require(Objects.equals(prototype.getXid(),"rollback:xid")&&prototype.getBranchId()==42,"Actual original decoded identity differs");
            List<String> modes=new ArrayList<>(List.of("matching_after"));if(prototype.getSqlUndoLogs().size()==1)modes.addAll(List.of("already_before","dirty"));
            for(String mode:modes){
                BranchUndoLog branch=parser.decode(payload(input));
                try(Connection c=connect(args,profile)){
                    for(String name:List.of("undo_child","undo_business","undo_work","undo_log"))execute(c,"DROP TABLE IF EXISTS "+name);
                    execute(c,Files.readString(Path.of(args[5])));execute(c,"CREATE TABLE undo_work(id INT PRIMARY KEY,v VARCHAR(1000)) ENGINE=InnoDB");
                    for(Object sql:(List<?>)input.get("initial"))execute(c,(String)sql);Map<String,Object> initial=business(c);
                    if(!Objects.equals(mode,"already_before"))for(Object sql:(List<?>)input.get("statements"))execute(c,(String)sql);
                    insertRows(c,(List<?>)input.get("rows"));
                    execute(c,"INSERT INTO undo_log VALUES(990007,'observer:other','untouched',X'0033',0,now(6),now(6))");
                    for(SQLUndoLog log:branch.getSqlUndoLogs()){TableMeta meta=oracle.fetchSchema(c,log.getTableName());log.setTableMeta(meta);}
                    boolean dirtyMustReject=Objects.equals(mode,"dirty")&&!DataCompareUtils.isRecordsEquals(branch.getSqlUndoLogs().get(0).getBeforeImage(),branch.getSqlUndoLogs().get(0).getAfterImage()).getResult();
                    if(Objects.equals(mode,"dirty"))dirty(c,branch.getSqlUndoLogs().get(0));Map<String,Object> before=business(c);List<Map<String,Object>> beforeUndo=undoSnapshot(c);List<Map<String,Object>> sentinel=beforeUndo.stream().filter(r->Objects.equals(r.get("xid"),"observer:other")).toList();require(sentinel.size()==1,"Other branch sentinel missing");c.setAutoCommit(false);List<Map<String,Object>> events=new ArrayList<>();boolean rejected=false;String errorType=null;
                    SourceManager.enter();try{
                        List<SQLUndoLog> logs=new ArrayList<>(branch.getSqlUndoLogs());Collections.reverse(logs);DialectConnection proxy=new DialectConnection(tracked(c,events));
                        for(SQLUndoLog log:logs){AbstractUndoExecutor executor=switch(log.getSqlType()){case INSERT->new MySQLUndoInsertExecutor(log);case UPDATE->new MySQLUndoUpdateExecutor(log);case DELETE->new MySQLUndoDeleteExecutor(log);default->throw new AssertionError("Original source case SQL type");};executor.executeOn(proxy);}
                        if(!Objects.equals(control,"skip_cleanup"))SourceManager.clean(tracked(c,events));c.commit();
                    }catch(SQLException error){rejected=true;errorType=error.getClass().getName();c.rollback();}
                    finally{SourceManager.exit();}
                    if(rejected&&Objects.equals(control,"drop_retained")){SourceManager.clean(c);c.commit();}
                    if(Objects.equals(mode,"dirty")){require(rejected==dirtyMustReject&&(!rejected||Objects.equals(errorType,"org.apache.seata.rm.datasource.undo.SQLUndoDirtyException")),"Original dirty/no-change behavior differs: "+errorType);require(before.equals(business(c)),"Original dirty rollback changed business");}
                    else{require(!rejected,"Original Undo unexpectedly rejected: "+errorType);require(initial.equals(business(c)),"Original source executor did not restore native initial rows");}
                    List<Map<String,Object>> afterUndo=undoSnapshot(c);require(rejected?beforeUndo.equals(afterUndo):sentinel.equals(afterUndo),rejected?"Original dirty Undo bytes changed":"Original Undo root/children cleanup mismatch");
                    Map<String,Object> result=new LinkedHashMap<>();result.put("case",input.get("case"));result.put("mode",mode);result.put("source_DML",events);result.put("dirty_rejected",rejected);result.put("error_type",errorType);result.put("status","MATCH");result.put("after",business(c));result.put("undo_before",beforeUndo);result.put("undo_after",afterUndo);result.put("other_branch_unchanged",true);result.put("root_children_deleted",!rejected);result.put("dirty_undo_exactly_retained",rejected);results.add(result);
                }
            }
        }
        Files.write(Path.of(args[4]),mapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(results));System.out.println("original_undo_executor_observations="+results.size()+" MATCH=true full_undo=false");System.exit(0);
    }
}
