import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.common.util.StringUtils;
import org.apache.seata.integration.tx.api.fence.constant.CommonFenceConstant;
import org.apache.seata.rm.tcc.api.BusinessActionContext;
import org.apache.seata.rm.tcc.api.BusinessActionContextUtil;

/** Executes unchanged pinned objects; output is observations, not a reimplementation. */
public final class ContextOracle {
    private ContextOracle() {}
    private static void observe(String name, String data) {
        try {
            BusinessActionContext context = BusinessActionContextUtil.getBusinessActionContext(
                    null, Long.MIN_VALUE, null, data);
            Map<String, Object> values = context.getActionContext();
            System.out.println(name + "\taccepted\t" + values.size() + "\t"
                    + Boolean.TRUE.equals(values.get("useTCCFence")) + "\t"
                    + context.getBranchId() + "\t" + Objects.isNull(context.getXid()) + "\t"
                    + Objects.isNull(context.getActionName()) + "\t"
                    + Objects.toString(values.get("x"), "-") + "\t"
                    + (Objects.isNull(data) ? "-" : StringUtils.isEmpty(data) ? "~" : Base64.getEncoder().encodeToString(data.getBytes(StandardCharsets.UTF_8))));
        } catch (RuntimeException exception) {
            System.out.println(name + "\trejected\t" + exception.getClass().getName() + "\t"
                    + Base64.getEncoder().encodeToString(data.getBytes(StandardCharsets.UTF_8)));
        }
    }
    public static void main(String[] args) {
        System.out.println("statuses\t" + CommonFenceConstant.STATUS_TRIED + "\t"
                + CommonFenceConstant.STATUS_COMMITTED + "\t" + CommonFenceConstant.STATUS_ROLLBACKED + "\t"
                + CommonFenceConstant.STATUS_SUSPENDED);
        StringBuilder blank = new StringBuilder("java_blank");
        for (int value = 0; value <= Character.MAX_VALUE; value++) {
            if (Character.isWhitespace((char) value)) blank.append('\t').append(value);
        }
        System.out.println(blank);
        observe("none", null);
        observe("empty", "");
        observe("blank", " \t\r\n\u001c\u2003");
        observe("missing", "{}");
        observe("null_context", "{\"actionContext\":null}");
        observe("true_flag", "{\"actionContext\":{\"useTCCFence\":true,\"value\":9223372036854775807}}");
        observe("string_flag", "{\"actionContext\":{\"useTCCFence\":\"true\"}}");
        observe("number_flag", "{\"actionContext\":{\"useTCCFence\":1}}");
        observe("huge_integer", "{\"actionContext\":{\"v\":1234567890123456789012345678901234567890}}");
        observe("decimal", "{\"actionContext\":{\"v\":1.23000000000000000000000000001}}");
        observe("duplicate_key", "{\"actionContext\":{\"x\":1,\"x\":2}}");
        observe("duplicate_context", "{\"actionContext\":{\"x\":1},\"actionContext\":{\"last\":true}}");
        observe("array_context", "{\"actionContext\":[]}");
        observe("number_context", "{\"actionContext\":1}");
        observe("string_context", "{\"actionContext\":\"bad\"}");
        observe("null_document", "null");
        observe("array_document", "[]");
        observe("nbsp", "\u00a0");
        observe("next_line", "\u0085");
        observe("figure_space", "\u2007");
        observe("narrow_nbsp", "\u202f");
    }
}
