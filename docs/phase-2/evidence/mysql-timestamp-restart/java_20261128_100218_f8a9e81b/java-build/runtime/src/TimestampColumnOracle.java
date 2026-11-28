package org.apache.seata.rm.datasource.sql.struct.cache;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Properties;
import java.util.StringJoiner;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.apache.seata.sqlparser.SQLType;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 固定原元数据/取行/parser源码的真实数据库格式对照，不表示全部Java案例完成。 */
public final class TimestampColumnOracle extends MysqlTableMetaCache {
    private static final String[] TABLES={"undo_timestamp","undo_timestamp_keys"};

    public static void main(String[] args) throws Exception {
        Path out=Path.of(args[0]);Files.createDirectories(out);
        String driver=args[1];String profile=args[2];
        Class.forName(driver.startsWith("5.")?"com.mysql.jdbc.Driver":"com.mysql.cj.jdbc.Driver");
        Properties properties=new Properties();
        properties.setProperty("user","root");
        properties.setProperty("password",Objects.requireNonNull(System.getenv("SEATA_MYSQL_PASSWORD")));
        properties.setProperty("useSSL","false");properties.setProperty("allowPublicKeyRetrieval","true");
        properties.setProperty("useUnicode","true");properties.setProperty("characterEncoding","UTF-8");
        properties.setProperty("useInformationSchema","true");properties.setProperty("serverTimezone","UTC");
        if("numeric_tinyint".equals(profile))properties.setProperty("tinyInt1isBit","false");
        String url="jdbc:mysql://127.0.0.1:"+System.getenv("SEATA_MYSQL_PORT")+"/"+System.getenv("SEATA_MYSQL_DATABASE");
        JacksonUndoLogParser.setZoneOffset(java.time.ZoneOffset.UTC);
        JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();
        ObjectMapper reportMapper=new ObjectMapper();
        Map<String,Object> report=new LinkedHashMap<>();
        List<Object> cases=new ArrayList<>();report.put("cases",cases);
        try(Connection connection=DriverManager.getConnection(url,properties)) {
            report.put("driver",connection.getMetaData().getDriverVersion());
            report.put("profile",profile);report.put("undo_timezone","UTC");report.put("jvm_timezone",java.util.TimeZone.getDefault().getID());report.put("database",connection.getMetaData().getDatabaseProductVersion());
            report.put("jackson",reportMapper.version().toString());
            try(Statement statement=connection.createStatement()) {
                statement.execute("SET NAMES utf8mb4");
                statement.execute("SET SESSION sql_mode='STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION,NO_BACKSLASH_ESCAPES',transaction_isolation='REPEATABLE-READ'");
            }
            TimestampColumnOracle cache=new TimestampColumnOracle();
            for(String table:TABLES) {
                TableMeta metadata=cache.fetchSchema(connection,table);
                StringJoiner order=new StringJoiner(",");
                for(String key:metadata.getPrimaryKeyMap().keySet())order.add("`"+key+"`");
                TableRecords records;
                try(Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT * FROM `"+table+"` ORDER BY "+order)) {
                    records=TableRecords.buildRecords(metadata,rows);
                }
                BranchUndoLog branch=new BranchUndoLog();branch.setXid("127.0.0.1:8091:42");branch.setBranchId(Long.MAX_VALUE);
                List<SQLUndoLog> logs=new ArrayList<>();branch.setSqlUndoLogs(logs);
                for(SQLType type:new SQLType[]{SQLType.INSERT,SQLType.UPDATE,SQLType.DELETE}) {
                    SQLUndoLog log=new SQLUndoLog();log.setSqlType(type);log.setTableName(table);
                    log.setBeforeImage(type==SQLType.INSERT?TableRecords.empty(metadata):records);
                    log.setAfterImage(type==SQLType.DELETE?TableRecords.empty(metadata):records);
                    logs.add(log);
                }
                byte[] encoded=parser.encode(branch);
                if(new String(encoded,java.nio.charset.StandardCharsets.UTF_8).contains("\"tableMeta\""))throw new AssertionError("transient metadata leaked");
                Files.write(out.resolve(table+".json"),encoded);
                BranchUndoLog decoded=parser.decode(encoded);
                if(decoded.getBranchId()!=Long.MAX_VALUE || !Objects.equals(decoded.getXid(),branch.getXid()) || decoded.getSqlUndoLogs().size()!=3)throw new AssertionError("original branch identity");
                for(int i=0;i<3;i++) {
                    SQLUndoLog before=branch.getSqlUndoLogs().get(i),after=decoded.getSqlUndoLogs().get(i);
                    if(before.getSqlType()!=after.getSqlType() || !Objects.equals(before.getTableName(),after.getTableName()))throw new AssertionError("SQL identity");
                    compare(before.getBeforeImage(),after.getBeforeImage());compare(before.getAfterImage(),after.getAfterImage());
                }
                Map<String,Object> item=new LinkedHashMap<>();item.put("table",table);item.put("rows",records.getRows().size());
                Map<String,String> nativeTypes=new LinkedHashMap<>();
                try(PreparedStatement statement=connection.prepareStatement("SELECT COLUMN_NAME,COLUMN_TYPE FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME=? ORDER BY ORDINAL_POSITION")) {
                    statement.setString(1,table);
                    try(ResultSet rows=statement.executeQuery()) {while(rows.next())nativeTypes.put(rows.getString(1),rows.getString(2));}
                }
                List<Object> columns=new ArrayList<>();item.put("columns",columns);
                for(ColumnMeta column:metadata.getAllColumns().values()) {
                    Map<String,Object> value=new LinkedHashMap<>();value.put("name",column.getColumnName());
                    value.put("jdbc_type",column.getDataType());value.put("type_name",column.getDataTypeName());
                    value.put("ordinal",column.getOrdinalPosition());value.put("precision",column.getColumnSize());
                    value.put("scale",column.getDecimalDigits());columns.add(value);
                    value.put("native_type",nativeTypes.get(column.getColumnName()));
                }
                List<Object> actualRows=new ArrayList<>();item.put("values",actualRows);
                for(Row row:records.getRows()) {
                    List<Object> fields=new ArrayList<>();actualRows.add(fields);
                    for(Field field:row.getFields()) {
                        Map<String,Object> value=new LinkedHashMap<>();Object data=field.getValue();
                        value.put("name",field.getName());value.put("key_type",field.getKeyType().name());value.put("jdbc_type",field.getType());
                        value.put("runtime_class",Objects.isNull(data)?null:data.getClass().getName());
                        value.put("exact_value",Objects.isNull(data)?null:data instanceof byte[]?Base64.getEncoder().encodeToString((byte[])data):data instanceof BigDecimal?((BigDecimal)data).toPlainString():data.toString());
                        fields.add(value);
                    }
                }
                List<String> beforeClasses=new ArrayList<>(),afterClasses=new ArrayList<>();
                for(SQLUndoLog log:decoded.getSqlUndoLogs()) {beforeClasses.add(log.getBeforeImage().getClass().getName());afterClasses.add(log.getAfterImage().getClass().getName());}
                item.put("before_runtime_classes",beforeClasses);item.put("after_runtime_classes",afterClasses);
                item.put("encoded_bytes",encoded.length);item.put("original_roundtrip",true);cases.add(item);
            }
        }
        byte[] defaultContent=parser.getDefaultContent();Files.write(out.resolve("default-content.bin"),defaultContent);
        if(!Arrays.equals(defaultContent,new byte[]{'{','}'}))throw new AssertionError("original default content bytes");
        BranchUndoLog empty=parser.decode(defaultContent);
        if(Objects.nonNull(empty.getXid()) || empty.getBranchId()!=0 || Objects.nonNull(empty.getSqlUndoLogs()))throw new AssertionError("original default content");
        report.put("default_content_roundtrip",true);
        Files.write(out.resolve("observations.json"),reportMapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(report));
        System.out.println("original_java_timestamp_cases=2 roundtrip=true driver="+driver+" profile="+profile);
    }

    private static void compare(TableRecords expected,TableRecords actual) {
        if(expected.getClass()!=actual.getClass())throw new AssertionError("original record runtime class changed");
        if(!Objects.equals(expected.getTableName(),actual.getTableName()) || expected.getRows().size()!=actual.getRows().size())throw new AssertionError("record shape");
        for(int i=0;i<expected.getRows().size();i++) {
            List<Field> left=expected.getRows().get(i).getFields(),right=actual.getRows().get(i).getFields();
            if(left.size()!=right.size())throw new AssertionError("field count");
            for(int j=0;j<left.size();j++) {
                Field a=left.get(j),b=right.get(j);Object av=a.getValue(),bv=b.getValue();
                if(a.getType()!=b.getType() || a.getKeyType()!=b.getKeyType() || !Objects.equals(a.getName(),b.getName()) || !Objects.deepEquals(av,bv))throw new AssertionError("exact original field roundtrip");
                if(Objects.nonNull(av) && av.getClass()!=bv.getClass())throw new AssertionError("original runtime class changed");
            }
        }
    }
}
