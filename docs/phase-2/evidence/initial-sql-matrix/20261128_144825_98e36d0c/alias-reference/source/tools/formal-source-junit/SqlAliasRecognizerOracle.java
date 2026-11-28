import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.sqlparser.SQLRecognizer;
import org.apache.seata.sqlparser.SQLUpdateRecognizer;
import org.apache.seata.sqlparser.druid.DruidDelegatingSQLRecognizerFactory;
import org.springframework.mock.env.MockEnvironment;

/** 固定原 Java Druid 识别器的别名、真实表和赋值列对照，不替换实现。 */
public final class SqlAliasRecognizerOracle {
    /** 参数为正式 SQL 矩阵和输出文件；输出六个新增用例的原识别结果。 */
    public static void main(String[] args) throws Exception {
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT,
                new MockEnvironment().withProperty("seata.config.type", "file")
                        .withProperty("seata.config.file.name", "file").withProperty("seata.registry.type", "file"));
        ObjectMapper mapper = new ObjectMapper();
        List<Map<String, Object>> results = new ArrayList<>();
        DruidDelegatingSQLRecognizerFactory factory = new DruidDelegatingSQLRecognizerFactory();
        for (JsonNode row : mapper.readTree(Files.readAllBytes(Path.of(args[0]))).path("accepted")) {
            String id = row.path("id").asText();
            if (!id.startsWith("alias_") && !id.equals("qualified_without_alias")) { continue; }
            List<SQLRecognizer> recognized = factory.create(row.path("sql").asText(), "mysql");
            if (recognized.size() != 1) { throw new IllegalStateException("single original recognizer required"); }
            SQLRecognizer value = recognized.get(0);
            Map<String, Object> result = new LinkedHashMap<>();
            result.put("id", id); result.put("table", value.getTableName());
            result.put("alias", value.getTableAlias()); result.put("type", value.getSQLType().toString());
            if (value instanceof SQLUpdateRecognizer) {
                result.put("columns", ((SQLUpdateRecognizer) value).getUpdateColumns());
            }
            results.add(result);
        }
        if (results.size() != 6) { throw new IllegalStateException("six alias/qualifier cases required"); }
        Files.write(Path.of(args[1]), mapper.writeValueAsBytes(results));
    }
}
