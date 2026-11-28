import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;

/** 用固定原 Java parser 读取正式 SQL 矩阵 Undo，并核对重编码后的全部业务字段。 */
public final class SqlMatrixUndoOracle {
    private static final ObjectMapper JSON = new ObjectMapper();

    /** 参数为现场目录；输出原 parser 的逐条读取及语义重编码结果。 */
    public static void main(String[] args) throws Exception {
        Path root = Path.of(args[0]);
        JacksonUndoLogParser.setZoneOffset(ZoneOffset.UTC);
        JacksonUndoLogParser parser = new JacksonUndoLogParser(); parser.init();
        List<Object> observations = new ArrayList<>();
        List<Path> files;
        try (java.util.stream.Stream<Path> stream = Files.walk(root)) {
            files = stream.filter(p -> p.getFileName().toString().endsWith(".undo.json")).sorted().toList();
        }
        for (Path path : files) {
            byte[] original = Files.readAllBytes(path);
            BranchUndoLog branch = parser.decode(original);
            byte[] encoded = parser.encode(branch);
            if (!normalize(JSON.readTree(original)).equals(normalize(JSON.readTree(encoded))))
                throw new IllegalStateException("Original Java parser changed SQL Undo fields: " + path);
            Path target = path.resolveSibling(path.getFileName().toString() + ".java.json");
            Files.write(target, encoded);
            observations.add(Map.of("path", root.relativize(path).toString().replace('\\', '/'),
                    "xid", branch.getXid(), "branch_id", branch.getBranchId(), "logs", branch.getSqlUndoLogs().size()));
        }
        Files.write(root.resolve("java-oracle.json"), JSON.writeValueAsBytes(Map.of("jackson", JSON.version().toString(), "cases", observations)));
    }

    private static JsonNode normalize(JsonNode value) {
        if (value.isObject()) {
            ObjectNode result = JSON.createObjectNode();
            value.fields().forEachRemaining(entry -> {
                if (!"@class".equals(entry.getKey())) result.set(entry.getKey(), normalize(entry.getValue()));
            });
            return result;
        }
        if (value.isArray()) {
            if (value.size() == 2 && value.get(0).isTextual() && value.get(0).asText().startsWith("java.util.") && value.get(1).isArray())
                return normalize(value.get(1));
            ArrayNode result = JSON.createArrayNode(); value.forEach(item -> result.add(normalize(item))); return result;
        }
        return value;
    }
}
