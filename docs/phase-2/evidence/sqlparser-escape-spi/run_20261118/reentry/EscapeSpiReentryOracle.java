import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Locale;
import org.apache.seata.common.loader.LoadLevel;
import org.apache.seata.sqlparser.EscapeHandler;
import org.apache.seata.sqlparser.EscapeHandlerFactory;
import org.apache.seata.sqlparser.EscapeSymbol;

public final class EscapeSpiReentryOracle {
    static int created;

    @LoadLevel(name = "test-reentrant")
    public static final class Reentrant extends EscapeSpiOracle.Base {
        final char symbol;

        public Reentrant() {
            if (++created == 1) {
                EscapeHandler nested = EscapeHandlerFactory.getEscapeHandler("TEST-REENTRANT");
                if (nested.getEscapeSymbol().getLeftSymbol() != 'I') {
                    throw new IllegalStateException("Unexpected nested provider");
                }
                symbol = 'O';
            } else {
                symbol = 'I';
            }
        }

        @Override public EscapeSymbol getEscapeSymbol() {
            return new EscapeSymbol(symbol);
        }
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        EscapeHandler outer = EscapeHandlerFactory.getEscapeHandler("test-reentrant");
        EscapeHandler inner = EscapeHandlerFactory.getEscapeHandler("TEST-REENTRANT");
        EscapeHandler fresh = EscapeHandlerFactory.getEscapeHandler("Test-Reentrant");
        Files.write(Path.of(args[0]), List.of(
                "outer\t" + outer.getEscapeSymbol().getLeftSymbol(),
                "inner\t" + inner.getEscapeSymbol().getLeftSymbol(),
                "same-inner-outer\t" + (inner == outer),
                "same-fresh-outer\t" + (fresh == outer),
                "created\t" + created));
    }
}
