package org.apache.seata.sqlparser.druid;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.apache.seata.sqlparser.util.DbTypeParser;

public final class JdbcDbTypeOracle {
    private static String encode(String input) {
        if (input == null) {
            return "null";
        }
        StringBuilder hex = new StringBuilder();
        for (char unit : input.toCharArray()) {
            hex.append(String.format(Locale.ROOT, "%04x", (int) unit));
        }
        return hex.toString();
    }

    public static void main(String[] args) throws Exception {
        DbTypeParser parser = new DruidDbTypeParserImpl();
        List<String> rows = new ArrayList<>();
        for (String input : Files.readAllLines(Path.of(args[0]))) {
            String url = null;
            if (!input.equals("null")) {
                char[] units = new char[input.length() / 4];
                for (int i = 0; i < units.length; i++) {
                    units[i] = (char) Integer.parseInt(input.substring(i * 4, i * 4 + 4), 16);
                }
                url = new String(units);
            }
            String actual = parser.parseFromJdbcUrl(url);
            rows.add(encode(url) + "\t" + (actual == null ? "null" : actual));
        }
        Files.write(Path.of(args[1]), rows);
    }
}
