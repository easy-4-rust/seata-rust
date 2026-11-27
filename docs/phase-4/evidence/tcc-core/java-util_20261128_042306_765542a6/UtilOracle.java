import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.common.json.JsonUtil;
import org.apache.seata.core.exception.TransactionException;
import org.apache.seata.core.model.BranchStatus;
import org.apache.seata.core.model.BranchType;
import org.apache.seata.core.model.ResourceManager;
import org.apache.seata.rm.DefaultResourceManager;
import org.apache.seata.rm.tcc.api.BusinessActionContext;
import org.apache.seata.rm.tcc.api.BusinessActionContextUtil;

/** 固定原 util 的调用与状态观察；ResourceManager 出口仅记录调用，不代表真实网络。 */
public final class UtilOracle {
    private static final List<Map<String, Object>> REPORTS = new ArrayList<>();
    private static boolean reject;

    private static void observe(String name, Boolean delay, Boolean updated, String before,
            String additions, boolean batch, boolean reportOnly, boolean failure) {
        REPORTS.clear();
        reject = failure;
        BusinessActionContext context = BusinessActionContextUtil.getBusinessActionContext(
                "original-xid", 9007199254740995L, "original-action",
                "{\"actionContext\":" + before + "}");
        context.enableActionContextTracking();
        context.setDelayReport(delay);
        context.setUpdated(updated);
        BusinessActionContextUtil.setContext(context);
        boolean result = false;
        boolean failed = false;
        try {
            if (reportOnly) {
                result = BusinessActionContextUtil.reportContext(context);
            } else if (batch) {
                result = BusinessActionContextUtil.addContext(
                        Objects.isNull(additions) ? null : JsonUtil.parseObject(additions, Map.class));
            } else {
                Object value = Objects.isNull(additions) ? null : JsonUtil.parseObject(additions, Object.class);
                result = BusinessActionContextUtil.addContext("value", value);
            }
        } catch (RuntimeException e) {
            failed = true;
        } finally {
            BusinessActionContextUtil.clear();
        }
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("name", name);row.put("delay", delay);row.put("initial_updated", updated);
        row.put("before", before);row.put("additions", additions);row.put("batch", batch);
        row.put("report_only", reportOnly);row.put("reject", failure);
        row.put("result", result);row.put("failed", failed);row.put("updated", context.getUpdated());
        row.put("values", context.getActionContext());row.put("reports", new ArrayList<>(REPORTS));
        row.put("holder_cleared", Objects.isNull(BusinessActionContextUtil.getContext()));
        System.out.println("UTIL\t" + JsonUtil.toJSONString(row));
    }

    public static void main(String[] args) {
        DefaultResourceManager.get();
        ResourceManager recorder = (ResourceManager) Proxy.newProxyInstance(
                ResourceManager.class.getClassLoader(), new Class<?>[]{ResourceManager.class},
                (proxy, method, values) -> {
                    if (method.getName().equals("getBranchType")) { return BranchType.TCC; }
                    if (!method.getName().equals("branchReport")) { throw new AssertionError(method.getName()); }
                    Map<String, Object> report = new LinkedHashMap<>();
                    report.put("type", values[0].toString());report.put("xid", values[1]);
                    report.put("branch_id", values[2].toString());report.put("status", values[3].toString());
                    report.put("application_data", values[4]);REPORTS.add(report);
                    if (reject) { throw new TransactionException("fixture report refusal"); }
                    return null;
                });
        DefaultResourceManager.mockResourceManager(BranchType.TCC, recorder);
        observe("single_null", false, null, "{}", "null", false, false, false);
        observe("single_absent", false, null, "{}", null, false, false, false);
        observe("batch_absent", false, true, "{}", null, true, false, false);
        observe("batch_empty", false, true, "{}", "{}", true, false, false);
        observe("same_pending", false, true, "{\"value\":1}", "1", false, false, false);
        observe("same_double", false, null, "{\"value\":1.0}", "1.00", false, false, false);
        observe("immediate_none", null, null, "{}", "1", false, false, false);
        observe("immediate_false", false, false, "{}", "2", false, false, false);
        observe("delayed", true, null, "{}", "3", false, false, false);
        observe("batch_merge", false, null, "{\"keep\":7}", "{\"a\":1,\"ignored\":null,\"b\":false}", true, false, false);
        observe("batch_nulls", false, true, "{\"a\":1}", "{\"a\":null}", true, false, false);
        observe("object", true, null, "{}", "{\"a\":1}", false, false, false);
        observe("array", false, null, "{}", "[1,2]", false, false, false);
        observe("object_string_same", false, null, "{\"value\":\"{\\\"a\\\":1}\"}", "{\"a\":1}", false, false, false);
        observe("rejected", false, null, "{}", "4", false, false, true);
        observe("report_null", false, null, "{\"a\":1}", null, false, true, false);
        observe("report_false", false, false, "{\"a\":1}", null, false, true, false);
        observe("report_true", true, true, "{\"a\":1}", null, false, true, false);
        observe("report_rejected", true, true, "{\"a\":1}", null, false, true, true);
    }
}
