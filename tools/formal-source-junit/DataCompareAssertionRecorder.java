package org.apache.seata.rm.datasource;

import org.junit.jupiter.api.Assertions;

/** 在额外观察副本里记录传给原 JUnit 断言的实际值。 */
public final class DataCompareAssertionRecorder {
    private static final ThreadLocal<String> METHOD = new ThreadLocal<>();

    private DataCompareAssertionRecorder() {
    }

    /** 保存当前测试方法及参数标签。 */
    public static void start(String label) {
        METHOD.set(label);
    }

    /** 记录实际布尔值后执行原 true 断言。 */
    public static void assertTrue(boolean value) {
        emit(value);
        Assertions.assertTrue(value);
    }

    /** 保留原消息的 true 断言。 */
    public static void assertTrue(boolean value, String message) {
        emit(value);
        Assertions.assertTrue(value, message);
    }

    /** 记录实际布尔值后执行原 false 断言。 */
    public static void assertFalse(boolean value) {
        emit(value);
        Assertions.assertFalse(value);
    }

    /** 记录原映射大小并执行原整数断言。 */
    public static void assertEquals(int expected, int value) {
        System.out.println("SEATA_COMPARE_JAVA\t" + METHOD.get() + "\t" + value + ":");
        Assertions.assertEquals(expected, value);
    }

    private static void emit(boolean value) {
        System.out.println("SEATA_COMPARE_JAVA\t" + METHOD.get() + "\t" + (value ? "1" : "0"));
    }
}
