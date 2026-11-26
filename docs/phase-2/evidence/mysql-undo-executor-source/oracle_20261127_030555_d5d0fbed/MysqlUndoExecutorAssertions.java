package seata.verification;

import java.util.Objects;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.function.Executable;

/** 仅在独立观察副本中记录原断言实参，始终委托未改动的 JUnit。 */
public final class MysqlUndoExecutorAssertions {
    private static final ThreadLocal<String> CASE = new ThreadLocal<>();
    private MysqlUndoExecutorAssertions() {}
    public static void start(String name) { CASE.set(name); }
    private static void emit(String value) {
        System.out.println("MYSQL_UNDO_EXECUTOR_ASSERT\t" + CASE.get() + "\t" +
            Base64.getEncoder().encodeToString(value.getBytes(StandardCharsets.UTF_8)));
    }
    public static void assertTrue(boolean actual) {
        emit("true:" + actual);
        Assertions.assertTrue(actual);
    }
    public static void assertFalse(boolean actual) {
        emit("false:" + actual);
        Assertions.assertFalse(actual);
    }
    public static void assertNull(Object actual) {
        emit("null:" + Objects.isNull(actual));
        Assertions.assertNull(actual);
    }
    public static void assertNotNull(Object actual) {
        emit("not_null:" + Objects.nonNull(actual));
        Assertions.assertNotNull(actual);
    }
    public static void assertEquals(Object expected, Object actual) {
        String value = (actual instanceof String || actual instanceof Number) ?
            actual.toString() : "object:" + Objects.equals(expected, actual);
        emit("equals:" + value);
        Assertions.assertEquals(expected, actual);
    }
    public static void assertNotEquals(Object expected, Object actual) {
        emit("not_equals:" + !Objects.equals(expected, actual));
        Assertions.assertNotEquals(expected, actual);
    }
    public static <T extends Throwable> T assertThrows(Class<T> type, Executable action) {
        T actual = Assertions.assertThrows(type, action);
        emit("throws:" + actual.getClass().getName());
        return actual;
    }
}
