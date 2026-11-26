import com.fasterxml.jackson.databind.ObjectMapper;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Timestamp;
import java.time.ZoneId;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.rm.datasource.sql.serial.SerialArray;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;

/** 仅消费自有固定测试资产；原parser会加载Java类型，不是任意不可信JSON服务入口。 */
public final class UndoDocumentPeer {
    public static void main(String[] args) throws Exception {
        JacksonUndoLogParser.setZoneOffset(ZoneId.of("UTC"));
        JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();
        ObjectMapper report=new ObjectMapper();
        Path input=Path.of(args[1]);Path output=Path.of(args[2]);Files.createDirectories(output);
        if("compare".equals(args[0])) {
            Path manifest=Path.of(args[3]);
            List<?> pairs=report.readValue(Files.readAllBytes(manifest),List.class);
            List<Object> results=new ArrayList<>();
            for(Object raw:pairs) {
                Map<?,?> pair=(Map<?,?>)raw;
                Path reference=Path.of((String)pair.get("reference"));Path candidate=Path.of((String)pair.get("candidate"));
                Object expected=branch(parser.decode(Files.readAllBytes(reference)));
                Object actual=branch(parser.decode(Files.readAllBytes(candidate)));
                if(!Objects.equals(expected,actual))throw new AssertionError("Java runtime type/value/container differential failed: "+pair.get("case"));
                Map<String,Object> result=new LinkedHashMap<>();result.put("case",pair.get("case"));result.put("status","MATCH");result.put("fingerprint",actual);results.add(result);
            }
            Files.write(output.resolve("comparison.json"),report.writerWithDefaultPrettyPrinter().writeValueAsBytes(results));
            System.out.println("original_java_differential_MATCH="+results.size());
        } else if("rewrite".equals(args[0])) {
            List<Object> results=new ArrayList<>();
            try(java.util.stream.Stream<Path> files=Files.list(input)) {
                for(Path file:files.filter(p->p.toString().endsWith(".json")).sorted().toList()) {
                    BranchUndoLog decoded=parser.decode(Files.readAllBytes(file));
                    Files.write(output.resolve(file.getFileName()),parser.encode(decoded));
                    Map<String,Object> result=new LinkedHashMap<>();result.put("case",file.getFileName().toString());result.put("fingerprint",branch(decoded));results.add(result);
                }
            }
            Files.write(output.resolve("fingerprints.report"),report.writerWithDefaultPrettyPrinter().writeValueAsBytes(results));
            System.out.println("original_java_rewrite_cases="+results.size());
        } else throw new IllegalArgumentException("owned peer mode");
    }

    private static Object branch(BranchUndoLog branch) throws Exception {
        if(Objects.isNull(branch))return null;
        Map<String,Object> out=new LinkedHashMap<>();out.put("class",branch.getClass().getName());out.put("xid",branch.getXid());
        out.put("branchId",Long.toString(branch.getBranchId()));
        List<Object> logs=Objects.isNull(branch.getSqlUndoLogs())?null:new ArrayList<>();out.put("sqlUndoLogs",logs);
        out.put("sqlUndoLogsClass",Objects.isNull(logs)?null:branch.getSqlUndoLogs().getClass().getName());
        if(Objects.nonNull(logs))for(SQLUndoLog log:branch.getSqlUndoLogs()) {
            if(Objects.isNull(log)) {logs.add(null);continue;}
            Map<String,Object> item=new LinkedHashMap<>();item.put("class",log.getClass().getName());
            item.put("sqlType",Objects.isNull(log.getSqlType())?null:log.getSqlType().name());item.put("tableName",log.getTableName());
            item.put("before",records(log.getBeforeImage()));item.put("after",records(log.getAfterImage()));logs.add(item);
        }
        return out;
    }
    private static Object records(TableRecords records) throws Exception {
        if(Objects.isNull(records))return null;
        Map<String,Object> out=new LinkedHashMap<>();out.put("class",records.getClass().getName());out.put("tableName",records.getTableName());
        List<Object> rows=Objects.isNull(records.getRows())?null:new ArrayList<>();out.put("rows",rows);
        out.put("rowsClass",Objects.isNull(rows)?null:records.getRows().getClass().getName());
        if(Objects.nonNull(rows))for(Row row:records.getRows()) {
            if(Objects.isNull(row)) {rows.add(null);continue;}
            Map<String,Object> rowOut=new LinkedHashMap<>();rowOut.put("class",row.getClass().getName());
            List<Object> fields=Objects.isNull(row.getFields())?null:new ArrayList<>();rowOut.put("fields",fields);
            rowOut.put("fieldsClass",Objects.isNull(fields)?null:row.getFields().getClass().getName());
            if(Objects.nonNull(fields))for(Field field:row.getFields()) {
                if(Objects.isNull(field)) {fields.add(null);continue;}
                Map<String,Object> item=new LinkedHashMap<>();item.put("class",field.getClass().getName());item.put("name",field.getName());
                item.put("keyType",Objects.isNull(field.getKeyType())?null:field.getKeyType().name());item.put("jdbcType",field.getType());item.put("value",value(field.getValue()));fields.add(item);
            }
            rows.add(rowOut);
        }
        return out;
    }
    private static Object value(Object value) throws Exception {
        if(Objects.isNull(value))return null;
        Map<String,Object> out=new LinkedHashMap<>();out.put("class",value.getClass().getName());
        if(value instanceof byte[])out.put("data",Base64.getEncoder().encodeToString((byte[])value));
        else if(value instanceof BigDecimal) {
            BigDecimal number=(BigDecimal)value;out.put("unscaled",number.unscaledValue().toString());out.put("scale",number.scale());
        } else if(value instanceof Timestamp) {
            Timestamp time=(Timestamp)value;out.put("millis",Long.toString(time.getTime()));out.put("nanos",time.getNanos());
        } else if(value instanceof SerialArray) {
            SerialArray array=(SerialArray)value;out.put("baseType",array.getBaseType());out.put("baseTypeName",array.getBaseTypeName());
            List<Object> elements=new ArrayList<>();for(Object item:array.getElements())elements.add(value(item));out.put("elements",elements);
        } else if(value instanceof javax.sql.rowset.serial.SerialBlob) {
            javax.sql.rowset.serial.SerialBlob blob=(javax.sql.rowset.serial.SerialBlob)value;out.put("data",Base64.getEncoder().encodeToString(blob.getBytes(1,(int)blob.length())));
        } else if(value instanceof javax.sql.rowset.serial.SerialClob) {
            javax.sql.rowset.serial.SerialClob clob=(javax.sql.rowset.serial.SerialClob)value;out.put("data",clob.getSubString(1,(int)clob.length()));
        } else out.put("data",value.toString());
        return out;
    }
}
