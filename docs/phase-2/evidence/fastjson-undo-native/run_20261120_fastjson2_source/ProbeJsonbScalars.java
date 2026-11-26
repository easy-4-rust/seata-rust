import com.alibaba.fastjson2.JSONB;
import java.util.HexFormat;
class ProbeJsonbScalars {
    public static void main(String[] args) {
        for (Object value : new Object[] {
            -262145, -262144, -2049, -2048, -17, -16, 47, 48, 2047, 2048, 262143, 262144,
            -262145L, -262144L, -2049L, -2048L, -9L, -8L, 15L, 16L, 2047L, 2048L, 262143L, 262144L,
            Long.MIN_VALUE, Long.MAX_VALUE,
            "", "é", "é".repeat(48), "中", "🐈", "a中b", "ab🐈c", "中".repeat(47), "abcdef中",
        }) {
            byte[] encoded = JSONB.toBytes(value);
            System.out.println(value.getClass().getSimpleName() + "\t" + value + "\t" + HexFormat.of().formatHex(encoded));
        }
    }
}
