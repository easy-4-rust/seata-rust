import com.alibaba.fastjson.JSON;
import com.alibaba.fastjson.serializer.SerializerFeature;

/** 记录固定 Fastjson 原生文本写入器的编码。 */
public final class FastjsonUndoEscapingOracle {
    public static void main(String[] args) {
        String value = new String(new char[]{
                0, 1, 8, 9, 10, 11, 12, 13, 14, 31, 32, 34, 47, 92, 126,
                127, 128, 159, 160, 0x2028, 0x2029, 0x87f9
        });
        System.out.println("SEATA_FASTJSON_ESCAPE\t" + JSON.toJSONString(
                value, SerializerFeature.WriteClassName, SerializerFeature.WriteDateUseDateFormat));
        Object overflow = JSON.parse("9223372036854775808L");
        System.out.println("SEATA_FASTJSON_LONG_OVERFLOW\t" + overflow.getClass().getName());
    }
}
