import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.apache.seata.common.loader.LoadLevel;
import org.apache.seata.common.loader.Scope;
import org.apache.seata.sqlparser.EscapeHandler;
import org.apache.seata.sqlparser.EscapeHandlerFactory;
import org.apache.seata.sqlparser.EscapeSymbol;
import org.apache.seata.sqlparser.struct.TableMeta;

public final class EscapeSpiOracle {
    static int winning;
    static int losing;
    static int prototype;

    public abstract static class Base implements EscapeHandler {
        @Override public boolean checkIfKeyWords(String name) { return true; }
        @Override public boolean checkIfNeedEscape(String name, TableMeta table) { return true; }
    }

    @LoadLevel(name = "test-singleton", order = Integer.MAX_VALUE)
    public static final class Winning extends Base {
        public Winning() { winning++; }
        @Override public EscapeSymbol getEscapeSymbol() { return new EscapeSymbol(']'); }
    }

    @LoadLevel(name = "test-singleton", order = Integer.MIN_VALUE)
    public static final class Losing extends Base {
        public Losing() { losing++; }
    }

    @LoadLevel(name = "test-prototype", scope = Scope.PROTOTYPE)
    public static final class Prototype extends Base {
        public Prototype() { prototype++; }
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        List<String> rows = new ArrayList<>();
        EscapeHandler a = EscapeHandlerFactory.getEscapeHandler("test-singleton");
        EscapeHandler b = EscapeHandlerFactory.getEscapeHandler("TEST-SINGLETON");
        rows.add("winner\t" + a.getEscapeSymbol().getLeftSymbol());
        rows.add("singleton-identity\t" + (a == b));
        rows.add("singleton-count\t" + winning);
        rows.add("losing-count\t" + losing);
        EscapeHandler p = EscapeHandlerFactory.getEscapeHandler("test-prototype");
        rows.add("prototype-same-raw\t" + (p == EscapeHandlerFactory.getEscapeHandler("test-prototype")));
        rows.add("prototype-case-variant\t" + (p == EscapeHandlerFactory.getEscapeHandler("TEST-PROTOTYPE")));
        rows.add("prototype-count\t" + prototype);
        Files.write(Path.of(args[0]), rows);
    }
}
