import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.apache.seata.rm.datasource.util.JdbcUtils;

public final class JdbcRmTypeOracle {
    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        List<String> results = new ArrayList<>();
        for (String input : Files.readAllLines(Path.of(args[0]))) {
            String url = null;
            if (!input.equals("null")) {
                char[] units = new char[input.length() / 4];
                for (int i = 0; i < units.length; i++) {
                    units[i] = (char) Integer.parseInt(input.substring(i * 4, i * 4 + 4), 16);
                }
                url = new String(units);
            }
            String observed;
            try {
                observed = JdbcUtils.getDbType(url);
            } catch (RuntimeException error) {
                observed = error.getClass().getName();
            }
            results.add(input + "\t" + observed);
        }
        Files.write(Path.of(args[1]), results);
    }
}
