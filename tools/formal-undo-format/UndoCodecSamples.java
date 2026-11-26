import com.fasterxml.jackson.databind.ObjectMapper;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Timestamp;
import java.sql.Types;
import java.time.LocalDateTime;
import java.time.ZoneId;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.sql.rowset.serial.SerialBlob;
import javax.sql.rowset.serial.SerialClob;
import org.apache.seata.rm.datasource.sql.serial.SerialArray;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.KeyType;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.apache.seata.sqlparser.SQLType;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 固定原源断言值的对象图探针；不冒充JUnit执行或全部源案例迁移。 */
public final class UndoCodecSamples {
    public static void main(String[] args) throws Exception {
        JacksonUndoLogParser.setZoneOffset(ZoneId.of("UTC"));
        JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();
        Path output=Path.of(args[0]);Files.createDirectories(output);
        List<Object> sources=new ArrayList<>();
        Object[][] fields={
            {"bigint",Types.BIGINT,Long.MAX_VALUE},
            {"decimal",Types.DECIMAL,new BigDecimal("55555555555555555555.55555555555555555555")},
            {"double",Types.DOUBLE,999999.999999999},
            {"timestamp-ms",Types.TIMESTAMP,Timestamp.valueOf("2021-05-18 17:23:22.111")},
            {"timestamp-micro",Types.TIMESTAMP,Timestamp.valueOf("2019-08-10 10:49:26.926554")},
            {"blob",Types.BLOB,new SerialBlob("hello".getBytes(StandardCharsets.UTF_8))},
            {"clob",Types.CLOB,new SerialClob("hello".toCharArray())},
            {"local-ms",Types.DATE,LocalDateTime.of(2021,5,18,17,23,22,111000000)},
            {"local-micro",Types.DATE,LocalDateTime.of(2021,5,18,17,23,22,222333000)}
        };
        for(Object[] data:fields) {
            String name=(String)data[0];
            write(parser,output,name,one(new Field(name,(Integer)data[1],data[2])),sources,
                "JacksonUndoLogParserTest#encode (field assertion values wrapped in a branch probe)");
        }
        for(boolean nullable:new boolean[]{false,true}) {
            SerialArray array=new SerialArray();array.setBaseType(Types.BIGINT);array.setBaseTypeName("int8");
            array.setElements(nullable?new Object[]{1L,null,3L}:new Object[]{1L,2L,3L,4L,5L});
            write(parser,output,nullable?"array-nulls":"array-longs",one(new Field("dept_ids",Types.ARRAY,array)),sources,
                "JacksonUndoLogParserTest#testSerializeAndDeserializeSerialArray"+(nullable?"WithNulls":"")+" (same elements, bean construction)");
        }
        Timestamp timestamp=new Timestamp(Integer.MAX_VALUE+1L);timestamp.setNanos(999999);
        Field field=new Field("gmt_create",Types.TIMESTAMP,timestamp);field.setKeyType(KeyType.PRIMARY_KEY);
        BranchUndoLog inherited=one(field);inherited.setXid("192.168.0.1:8091:123456");inherited.setBranchId(123457);
        inherited.getSqlUndoLogs().get(0).setBeforeImage(TableRecords.empty(new TableMeta()));
        inherited.getSqlUndoLogs().get(0).getAfterImage().setTableName("test");
        write(parser,output,"inherited-timestamp",inherited,sources,"BaseUndoLogParserTest#testTimestampEncodeAndDecode (object graph)");
        BranchUndoLog branch=new BranchUndoLog();branch.setXid("xid:xxx");branch.setBranchId(641789253L);
        SQLUndoLog item=new SQLUndoLog();item.setSqlType(SQLType.UPDATE);
        TableMeta meta=new TableMeta();meta.setTableName("product");
        TableRecords before=new TableRecords(meta);Row a=new Row();
        a.add(new Field("id",Types.INTEGER,1));a.add(new Field("name",Types.VARCHAR,"SEATA"));a.add(new Field("since",Types.VARCHAR,"2014"));before.add(a);
        TableRecords after=new TableRecords(meta);Row b=new Row();
        b.add(new Field("id",Types.INTEGER,1));b.add(new Field("name",Types.VARCHAR,"SEATA_IO"));b.add(new Field("since",Types.VARCHAR,"2014"));after.add(b);
        item.setBeforeImage(before);item.setAfterImage(after);branch.setSqlUndoLogs(new ArrayList<>(List.of(item)));
        write(parser,output,"branch-source",branch,sources,"BranchUndoLogTest#testEncodeUndoLog (same object graph; explicit Jackson profile)");
        for(String number:new String[]{"1E+2147483647","1E-2147483647","1.0E-2147483646","1.0E+2147483647"}) {
            write(parser,output,"decimal-scale-"+sources.size(),one(new Field("scale",Types.DECIMAL,new BigDecimal(number))),sources,
                "Rust obligation: original Java accepted BigDecimal scale boundary");
        }
        List<Object> rejected=new ArrayList<>();
        for(String number:new String[]{"1E+2147483648","1E-2147483648","1.0E-2147483647","0E+9999999999999999999999999999"}) {
            // 只把本工具已知的数值常量放进自有JSON，绝不转发不可信类标签给原Java。
            String document=new String(parser.encode(one(new Field("scale",Types.DECIMAL,new BigDecimal("1.25")))),StandardCharsets.UTF_8);
            document=document.replace("1.25",number);
            boolean refused=false;
            try {parser.decode(document.getBytes(StandardCharsets.UTF_8));} catch(RuntimeException error) {refused=true;}
            if(!refused)throw new AssertionError("Original parser accepted out-of-range BigDecimal scale");
            Map<String,Object> result=new LinkedHashMap<>();result.put("number",number);result.put("original_parser_rejected",true);rejected.add(result);
        }
        ObjectMapper mapper=new ObjectMapper();
        Files.write(output.resolve("source-probes.report"),mapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(sources));
        Files.write(output.resolve("decimal-rejections.report"),mapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(rejected));
        System.out.println("source_value_probes="+sources.size()+" original_decimal_rejections="+rejected.size());
    }
    private static BranchUndoLog one(Field field) {
        Row row=new Row();row.add(field);TableRecords records=new TableRecords();records.setTableName("probe");records.add(row);
        SQLUndoLog log=new SQLUndoLog();log.setAfterImage(records);
        BranchUndoLog branch=new BranchUndoLog();branch.setSqlUndoLogs(new ArrayList<>(List.of(log)));return branch;
    }
    private static void write(JacksonUndoLogParser parser,Path output,String name,BranchUndoLog branch,List<Object> report,String source) throws Exception {
        byte[] encoded=parser.encode(branch);parser.decode(encoded);
        Files.write(output.resolve("probe-"+name+".json"),encoded);
        report.add(Map.of("case","probe-"+name+".json","source",source,"original_JUnit_method_executed",false));
    }
}
