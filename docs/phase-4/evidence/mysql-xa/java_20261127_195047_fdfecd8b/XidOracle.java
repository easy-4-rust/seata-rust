import java.util.HexFormat;
import java.util.Objects;
import org.apache.seata.rm.datasource.xa.XAXid;
import org.apache.seata.rm.datasource.xa.XAXidBuilder;

/** 调用固定原 Java 对象生成 XA 原字节向量；不重写其编码。 */
public final class XidOracle {
    public static void main(String[] args) {
        String[] globals = {null, "", "tc:1", "tc:中'\\\n", "root:0", "signed:negative", "signed:min", "signed:max"};
        long[] branches = {0L, 0L, 17L, 9007199254740995L, 0L, -17L, Long.MIN_VALUE, Long.MAX_VALUE};
        for (int i = 0; i < globals.length; i++) {
            XAXid value = XAXidBuilder.build(globals[i], branches[i]);
            System.out.println((Objects.isNull(value.getGlobalXid()) ? "0" : "1") + "\t"
                    + value.getBranchId() + "\t" + value.getFormatId() + "\t"
                    + HexFormat.of().formatHex(value.getGlobalTransactionId()) + "\t"
                    + HexFormat.of().formatHex(value.getBranchQualifier()));
        }
    }
}
