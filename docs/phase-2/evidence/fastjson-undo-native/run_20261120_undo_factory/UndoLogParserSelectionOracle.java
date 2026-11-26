import java.util.Objects;
import org.apache.seata.common.loader.EnhancedServiceLoader;
import org.apache.seata.rm.datasource.undo.UndoLogParser;
import org.apache.seata.rm.datasource.undo.UndoLogParserFactory;

/** 固定 Java 解析器名称解析与实例共享的补充观测。 */
public final class UndoLogParserSelectionOracle {
    public static void main(String[] args) {
        System.out.println("DEFAULT\t" + UndoLogParserFactory.getInstance().getClass().getSimpleName());
        for (String name : new String[]{"fastjson", "jackson", "protostuff", "kryo", "fastjson2", "fory",
                "FURY", "JaCkSoN", "jacKson", "Kryo", "fst", "FST", "fſt", "adadad", "", " jackson", null}) {
            for (boolean factory : new boolean[]{true, false}) {
                String result;
                try {
                    var parser = load(name, factory);
                    result = parser.getClass().getSimpleName() + "\t" + (parser == load(name, factory));
                } catch (RuntimeException error) {
                    result = error.getClass().getSimpleName();
                }
                System.out.println((factory ? "FACTORY" : "SPI") + "\t" + Objects.toString(name, "<null>") + "\t" + result);
            }
        }
        System.out.println("ALIAS\t" + (UndoLogParserFactory.getInstance("FURY") == UndoLogParserFactory.getInstance("fory")));
    }

    private static UndoLogParser load(String name, boolean factory) {
        return factory ? UndoLogParserFactory.getInstance(name) : EnhancedServiceLoader.load(UndoLogParser.class, name);
    }
}
