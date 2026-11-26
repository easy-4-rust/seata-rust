import java.sql.Driver;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import org.apache.seata.rm.datasource.util.JdbcUtils;

/** 读取原生产 JdbcUtils 的四个原始驱动用例的结果及异常身份。 */
public final class JdbcDriverCaseOracle {
    private JdbcDriverCaseOracle() {
    }

    /** 输出四个原用例的实际观察，供 Rust 逐案对照。 */
    public static void main(String[] args) {
        observe("testLoadDriverWithValidH2Driver", "org.h2.Driver");
        observe("testLoadDriverWithInvalidDriverClass", "com.invalid.NonExistentDriver");
        observe("testLoadDriverWithNullClassName", null);
        observe("testLoadDriverWithEmptyClassName", "");
    }

    private static void observe(String name, String input) {
        try {
            Driver driver = JdbcUtils.loadDriver(input);
            System.out.println(name + "\tdriver:" + driver.getClass().getName() + ":" + (driver instanceof org.h2.Driver));
        } catch (Exception error) {
            List<String> chain = new ArrayList<>();
            for (Throwable cause = error; Objects.nonNull(cause); cause = cause.getCause()) {
                chain.add(cause.getClass().getName());
            }
            System.out.println(name + "\t" + String.join(">", chain));
        }
    }
}
