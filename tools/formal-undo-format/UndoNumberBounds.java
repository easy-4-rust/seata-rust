import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;

/** 固定Jackson数字读取约束的自有常量探针；不消费外部类名或数值。 */
public final class UndoNumberBounds {
    public static void main(String[] args) throws Exception {
        JacksonUndoLogParser parser=new JacksonUndoLogParser();parser.init();
        Path output=Path.of(args[0]);Files.createDirectories(output);
        Map<String,String> numbers=new LinkedHashMap<>();
        numbers.put("integer-1000","1".repeat(1000));
        numbers.put("integer-1001","1".repeat(1001));
        numbers.put("decimal-integer-1000","1".repeat(1000));
        numbers.put("decimal-integer-1001","1".repeat(1001));
        numbers.put("negative-1000","-"+"1".repeat(1000));
        numbers.put("negative-1001","-"+"1".repeat(1001));
        numbers.put("fraction-1000","0."+"1".repeat(1000));
        numbers.put("fraction-1001","0."+"1".repeat(1001));
        numbers.put("float-999","1."+"1".repeat(998));
        numbers.put("float-1000","1."+"1".repeat(999));
        numbers.put("float-1001","1."+"1".repeat(1000));
        numbers.put("negative-float-1000","-1."+"1".repeat(999));
        numbers.put("exponent-1000","1."+"1".repeat(998)+"E+0");
        numbers.put("exponent-1001","1."+"1".repeat(999)+"E+0");
        List<Object> results=new ArrayList<>();
        for(Map.Entry<String,String> entry:numbers.entrySet()) {
            String type=!entry.getKey().startsWith("decimal")&&(entry.getKey().contains("integer")||entry.getKey().startsWith("negative-1"))?"java.math.BigInteger":"java.math.BigDecimal";
            String document="{\"@class\":\"org.apache.seata.rm.datasource.undo.BranchUndoLog\",\"sqlUndoLogs\":[\"java.util.ArrayList\",[{\"@class\":\"org.apache.seata.rm.datasource.undo.SQLUndoLog\",\"afterImage\":{\"@class\":\"org.apache.seata.rm.datasource.sql.struct.TableRecords\",\"rows\":[\"java.util.ArrayList\",[{\"@class\":\"org.apache.seata.rm.datasource.sql.struct.Row\",\"fields\":[\"java.util.ArrayList\",[{\"@class\":\"org.apache.seata.rm.datasource.sql.struct.Field\",\"value\":[\""+type+"\","+entry.getValue()+"]}]]}]]}}]]}";
            boolean accepted=true;String error=null;
            try {parser.decode(document.getBytes(StandardCharsets.UTF_8));} catch(RuntimeException failure) {accepted=false;error=failure.getClass().getName();}
            if(accepted)Files.writeString(output.resolve("number-"+entry.getKey()+".json"),document,StandardCharsets.UTF_8);
            Map<String,Object> result=new LinkedHashMap<>();result.put("case",entry.getKey());result.put("class",type);
            result.put("lexeme_bytes",entry.getValue().length());result.put("original_accepted",accepted);result.put("error_class",error);results.add(result);
        }
        Files.write(output.resolve("number-bounds.report"),new ObjectMapper().writerWithDefaultPrettyPrinter().writeValueAsBytes(results));
        System.out.println(new ObjectMapper().writeValueAsString(results));
    }
}
