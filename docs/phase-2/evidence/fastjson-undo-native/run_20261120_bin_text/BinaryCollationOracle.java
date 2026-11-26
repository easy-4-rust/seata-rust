import java.nio.charset.StandardCharsets;
import java.sql.DriverManager;
import java.sql.ResultSet;
import java.util.HexFormat;
import java.util.Properties;

public class BinaryCollationOracle {
    public static void main(String[] args) throws Exception {
        Properties options = new Properties();
        options.setProperty("user", "root");
        options.setProperty("password", System.getenv("SEATA_MYSQL_PASSWORD"));
        options.setProperty("sslMode", "DISABLED");
        options.setProperty("allowPublicKeyRetrieval", "true");
        try (var db = DriverManager.getConnection("jdbc:mysql://127.0.0.1:43318/undo_wsl_bin_text", options);
             var query = db.prepareStatement("SELECT id,note,tag,legacy,payload FROM bin_text_image");
             var row = query.executeQuery()) {
            if (!row.next()) {
                throw new IllegalStateException("Missing restored business row");
            }
            System.out.println(db.getMetaData().getDriverVersion());
            String[] expected = { "A\uD801\uDC00", "Cafe\u0301", "Alpha ", "\u539f\u5b57\u7b26" };
            for (int i = 0; i < expected.length; i++) {
                Object value = row.getObject(i + 1);
                if (!(value instanceof String) || !expected[i].equals(value)) {
                    throw new IllegalStateException("Text type or exact value differs at column " + (i + 1));
                }
                report(row, i + 1, ((String) value).getBytes(StandardCharsets.UTF_8));
            }
            Object value = row.getObject(5);
            if (!(value instanceof byte[]) || !java.util.Arrays.equals((byte[]) value, new byte[]{ 0, (byte) 255 }) || row.next()) {
                throw new IllegalStateException("Binary type, bytes or row count differs");
            }
            report(row, 5, (byte[]) value);
        }
    }
    private static void report(ResultSet row, int column, byte[] bytes) throws Exception {
        System.out.println(row.getMetaData().getColumnName(column) + "\t" +
                row.getMetaData().getColumnTypeName(column) + "\t" + HexFormat.of().formatHex(bytes));
    }
}
