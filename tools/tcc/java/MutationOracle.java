import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.common.json.JsonUtil;
import org.apache.seata.rm.tcc.api.BusinessActionContext;
import org.apache.seata.rm.tcc.api.BusinessActionContextUtil;

/** 固定原 Java 对象的变更观察器，不替代网络或持久报告。 */
public final class MutationOracle {
    private MutationOracle() {}

    private static void emit(Map<String, Object> row) {
        System.out.println("MUTATION\t" + JsonUtil.toJSONString(row));
    }

    private static void pair(String before, String after) {
        Object previous = JsonUtil.parseObject(before, Object.class);
        Object next = JsonUtil.parseObject(after, Object.class);
        BusinessActionContext context = new BusinessActionContext("original-xid", "9007199254740995", new HashMap<>());
        context.getActionContext().put("value", previous);
        context.enableActionContextTracking();
        boolean changed = context.addActionContext("value", next);
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("kind", "pair");
        row.put("before", before);
        row.put("after", after);
        row.put("before_class", Objects.isNull(previous) ? "null" : previous.getClass().getName());
        row.put("after_class", Objects.isNull(next) ? "null" : next.getClass().getName());
        row.put("changed", changed);
        row.put("updated", context.getUpdated());
        emit(row);
    }

    private static void state(String name, BusinessActionContext context) {
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("kind", "state");
        row.put("name", name);
        row.put("updated", context.getUpdated());
        row.put("delay_report", context.getDelayReport());
        row.put("values", context.getActionContext());
        row.put("xid", context.getXid());
        row.put("branch_id", Long.toString(context.getBranchId()));
        emit(row);
    }

    public static void main(String[] args) {
        String[][] pairs = {
            {"1","1"},{"0","-0"},{"1","1.0"},{"1.0","1.00"},{"1.0","1e0"},
            {"1e3","1000.0"},{"1e3","1E+3"},{"0.0","-0.0"},{"1.00e2","100.0"},
            {"2147483647","2147483648"},{"9223372036854775807","9223372036854775808"},
            {"1234567890123456789012345678901234567890","1234567890123456789012345678901234567890"},
            {"1.23000000000000000000000000001","1.23000000000000000000000000002"},
            {"1e10000","1E+10000"},{"1e-10000","1e-10000"},
            {"\"a\"","\"\\u0061\""},{"true","true"},{"true","false"},
            {"{\"a\":1,\"b\":2}","{\"b\":2,\"a\":1}"},
            {"[1,2]","[1,2]"},{"[1,2]","[2,1]"},{"[1.0]","[1.00]"},
            {"{\"a\":1}","{\"a\":2}"},{"{\"a\":null}","{}"},
            {"{\"a\":[1,2]}","{\"a\":[2,1]}"},{"1e10000","2e10000"},{"-1e-10000","0.0"},
            {"null","null"},{"1","null"},{"null","1"}
        };
        for (String[] values : pairs) pair(values[0], values[1]);
        BusinessActionContext context = new BusinessActionContext("original-xid", "9007199254740995", new HashMap<>());
        state("initial",context);
        context.getActionContext().put("a",1);state("untracked_put",context);
        context.enableActionContextTracking();state("enable",context);
        context.getActionContext().put("a",1);state("same_put",context);
        context.getActionContext().put("null",null);state("new_null",context);
        context.setUpdated(null);context.getActionContext().put("null",null);state("same_null",context);
        context.getActionContext().remove("missing");state("missing_remove",context);
        context.getActionContext().remove("null");state("remove_null",context);
        context.setUpdated(false);context.getActionContext().clear();state("clear",context);
        context.setUpdated(null);context.getActionContext().clear();state("clear_empty",context);
        context.setActionContext(new HashMap<>());state("replace_map",context);
        context.setDelayReport(true);context.addActionContext("delayed",2);state("delay_true",context);
        context.setDelayReport(false);state("delay_false",context);
        context.setUpdated(null);context.setDelayReport(null);state("reset",context);
        context.addActionContext("ignored",null);state("add_null",context);
        context.setDelayReport(true);BusinessActionContextUtil.setContext(context);
        boolean reported = BusinessActionContextUtil.addContext("object", Map.of("a",1));
        if (reported) throw new AssertionError("delay reported");
        state("util_delay_object",context);
        BusinessActionContextUtil.clear();
    }
}
