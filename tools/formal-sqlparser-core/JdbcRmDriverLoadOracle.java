import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Driver;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;
import org.apache.seata.rm.datasource.util.JdbcUtils;

/** 执行固定版原 JdbcUtils#loadDriver，记录驱动类型、加载器路径和异常因果。 */
public final class JdbcRmDriverLoadOracle {
    private enum Context { DELEGATE, ABSENT, MISSING, RETURNS_NULL, LINKAGE, SELECT_H2 }

    public static final class PrivateDriver extends org.h2.Driver {
        private PrivateDriver() {}
    }

    public static abstract class AbstractDriver extends org.h2.Driver {}

    public static final class MissingConstructor extends org.h2.Driver {
        public MissingConstructor(String argument) {}
    }

    public static final class AccessFailure extends IllegalAccessException {
        public AccessFailure() { super("constructor-access"); }
    }

    public static final class ConstructorAccess extends org.h2.Driver {
        public ConstructorAccess() throws AccessFailure { throw new AccessFailure(); }
    }

    public static final class ConstructorSql extends org.h2.Driver {
        public ConstructorSql() throws SQLException { throw new SQLException("constructor-sql"); }
    }

    public static final class ConstructorClassNotFound extends org.h2.Driver {
        public ConstructorClassNotFound() throws ClassNotFoundException {
            throw new ClassNotFoundException("constructor-missing");
        }
    }

    public static final class NotDriver {
        public NotDriver() {}
    }

    public static final class InitializerFailure extends org.h2.Driver {
        static {
            if (failure()) {
                throw new IllegalStateException("initializer");
            }
        }

        private static boolean failure() { return true; }
    }

    private record Case(String name, String className, Context context) {}

    public static void main(String[] args) throws Exception {
        String h2 = "org.h2.Driver";
        String missing = "com.invalid.NonExistentDriver";
        List<Case> cases = List.of(
            new Case("context-driver", h2, Context.DELEGATE),
            new Case("no-context", h2, Context.ABSENT),
            new Case("context-missing-default-driver", h2, Context.MISSING),
            new Case("context-null-class", h2, Context.RETURNS_NULL),
            new Case("context-selected-class", missing, Context.SELECT_H2),
            new Case("context-linkage", h2, Context.LINKAGE),
            new Case("missing-class", missing, Context.MISSING),
            new Case("empty-class", "", Context.MISSING),
            new Case("null-class-name", null, Context.ABSENT),
            new Case("private-constructor", PrivateDriver.class.getName(), Context.DELEGATE),
            new Case("abstract-class", AbstractDriver.class.getName(), Context.DELEGATE),
            new Case("missing-constructor", MissingConstructor.class.getName(), Context.DELEGATE),
            new Case("constructor-access-subclass", ConstructorAccess.class.getName(), Context.DELEGATE),
            new Case("constructor-sql", ConstructorSql.class.getName(), Context.DELEGATE),
            new Case("constructor-class-missing", ConstructorClassNotFound.class.getName(), Context.DELEGATE),
            new Case("wrong-driver-type", NotDriver.class.getName(), Context.DELEGATE),
            new Case("default-initializer-fails", InitializerFailure.class.getName(), Context.ABSENT)
        );
        List<String> rows = new ArrayList<>();
        for (Case input : cases) {
            rows.add(observe(input));
        }
        Files.write(Path.of(args[0]), rows, StandardCharsets.UTF_8);
    }

    private static String observe(Case input) {
        Thread thread = Thread.currentThread();
        ClassLoader parent = thread.getContextClassLoader();
        boolean[] called = {false};
        ClassLoader context = new ClassLoader(parent) {
            @Override
            public Class<?> loadClass(String name) throws ClassNotFoundException {
                called[0] = true;
                return switch (input.context()) {
                    case MISSING -> throw new ClassNotFoundException("context:" + name);
                    case RETURNS_NULL -> null;
                    case LINKAGE -> throw new LinkageError("context-linkage");
                    case SELECT_H2 -> org.h2.Driver.class;
                    default -> super.loadClass(name);
                };
            }
        };
        thread.setContextClassLoader(input.context() == Context.ABSENT ? null : context);
        String outcome;
        String message = "";
        try {
            Driver driver = JdbcUtils.loadDriver(input.className());
            outcome = "driver:" + driver.getClass().getName();
        } catch (Throwable error) {
            outcome = identity(error);
            message = error.getMessage();
        } finally {
            thread.setContextClassLoader(parent);
        }
        return input.name() + "\t" + called[0] + "\t" + outcome + "\t" + (message == null ? "<null>" : message);
    }

    private static String identity(Throwable error) {
        return error.getCause() == null
            ? error.getClass().getName()
            : error.getClass().getName() + ">" + identity(error.getCause());
    }
}
