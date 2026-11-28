package io.github.easy4j.seata.rust.jdbc;

import com.mysql.cj.MysqlType;
import com.mysql.cj.jdbc.result.ResultSetInternalMethods;
import com.mysql.cj.protocol.a.MysqlBinaryValueDecoder;
import com.mysql.cj.protocol.a.MysqlTextValueDecoder;
import com.mysql.cj.protocol.a.result.ByteArrayRow;
import com.mysql.cj.protocol.a.result.ResultsetRowsStatic;
import com.mysql.cj.result.DefaultColumnDefinition;
import com.mysql.cj.result.Field;
import com.mysql.cj.util.LazyString;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;
import java.sql.Types;
import java.util.Collections;
import java.util.Objects;

/** 使用真实 Connector/J 行类核对畸形协议值与 JDBC 数值 scale/物理 FSP 的差异。 */
public final class ExactTemporalValueReaderTest {
    private static int positive;
    private static int rejected;

    public static void main(String[] args) throws Exception {
        equal(Types.TIME, 6, false, ascii("-00:00:00.000001"), "-00:00:00.000001");
        equal(Types.TIME, 6, true, new byte[]{1,0,0,0,0,0,0,0,1,0,0,0}, "-00:00:00.000001");
        equal(Types.TIME, 6, true, new byte[]{0,0,0,0,0,0,0,0,1,0,0,0}, "00:00:00.000001");
        equal(Types.TIME, 6, true, new byte[]{1,34,0,0,0,22,59,59}, "-838:59:59.000000");
        equal(Types.TIME, 6, false, ascii("838:59:59.000000"), "838:59:59.000000");
        equal(Types.TIME, 4, true, new byte[]{0,0,0,0,0,12,34,56,8,(byte)226,1,0}, "12:34:56.1234");
        equal(Types.TIME, 6, true, new byte[0], "00:00:00.000000");
        equal(Types.TIME, 0, true, new byte[0], "00:00:00");
        equal(Types.DATE, 0, false, ascii("1582-10-05"), "1582-10-05");
        equal(Types.DATE, 0, true, new byte[]{46,6,10,5}, "1582-10-05");
        equal(Types.DATE, 0, true, new byte[]{(byte)232,3,1,1}, "1000-01-01");
        equal(Types.DATE, 0, true, new byte[]{15,39,12,31}, "9999-12-31");
        equal(Types.DATE, 0, false, null, null);
        equal(Types.TIME, 6, true, null, null);
        equal(Types.INTEGER, 0, false, ascii("1"), "delegated");
        for (String value : new String[]{"0000-00-00","1500-02-29","1000-02-30","0999-12-31","10000-01-01"}) bad(Types.DATE,0,false,ascii(value));
        bad(Types.DATE,0,true,new byte[0]);
        bad(Types.DATE,0,true,new byte[3]);
        bad(Types.DATE,0,false,new byte[]{(byte)255});
        for (String value : new String[]{"839:00:00","-839:00:00","838:59:59.000001","-838:59:59.000001","12:60:00","12:00:60","12:00:00.","12:00:00.0000001","12:0:00"}) bad(Types.TIME,6,false,ascii(value));
        bad(Types.TIME,0,false,ascii("12:00:00.000001"));
        bad(Types.TIME,7,true,new byte[0]);
        bad(Types.TIME,6,true,new byte[1]);
        bad(Types.TIME,6,true,new byte[]{2,0,0,0,0,0,0,0});
        bad(Types.TIME,6,true,new byte[]{0,0,0,0,0,24,0,0});
        bad(Types.TIME,6,true,new byte[]{0,0,0,0,0,0,0,0,64,66,15,0});
        bad(Types.TIME,6,true,new byte[]{0,34,0,0,0,22,59,59,1,0,0,0});
        System.out.println("EXACT_TEMPORAL_READER_CHECKED positive=" + positive + " rejected=" + rejected);
    }

    private static byte[] ascii(String value) { return value.getBytes(StandardCharsets.US_ASCII); }

    private static void equal(int jdbc, int scale, boolean binary, byte[] raw, Object expected) throws Exception {
        Object actual = ExactTemporalValueReader.read(rows(jdbc,scale,binary,raw),1);
        if (!Objects.equals(actual,expected)) throw new AssertionError("Exact temporal value differs");
        positive++;
    }

    private static void bad(int jdbc, int scale, boolean binary, byte[] raw) throws Exception {
        try { ExactTemporalValueReader.read(rows(jdbc,scale,binary,raw),1); }
        catch (SQLException expected) { rejected++; return; }
        throw new AssertionError("Malformed exact temporal value accepted");
    }

    private static ResultSet rows(int jdbc, int scale, boolean binary, byte[] raw) {
        LazyString name = new LazyString("v");
        Field field = new Field(name,name,name,name,name,26L,jdbc == Types.DATE ? 10 : 11,(short)0,scale,63,"US-ASCII",jdbc == Types.DATE ? MysqlType.DATE : MysqlType.TIME);
        DefaultColumnDefinition definition = new DefaultColumnDefinition(new Field[]{field});
        ByteArrayRow row = new ByteArrayRow(new byte[][]{raw},null,binary ? new MysqlBinaryValueDecoder() : new MysqlTextValueDecoder());
        ResultsetRowsStatic rows = new ResultsetRowsStatic(Collections.singletonList(row),definition);
        rows.setCurrentRow(0);
        ResultSetMetaData metadata = (ResultSetMetaData) Proxy.newProxyInstance(ResultSetMetaData.class.getClassLoader(),new Class<?>[]{ResultSetMetaData.class},(proxy,method,args)->{
            if ("getColumnType".equals(method.getName())) return jdbc;
            if ("getScale".equals(method.getName())) return 0;
            throw new SQLException("Unexpected metadata method");
        });
        return (ResultSet) Proxy.newProxyInstance(ResultSetInternalMethods.class.getClassLoader(),new Class<?>[]{ResultSetInternalMethods.class},(proxy,method,args)->{
            if ("getMetaData".equals(method.getName())) return metadata;
            if ("getColumnDefinition".equals(method.getName())) return definition;
            if ("getRows".equals(method.getName())) return rows;
            if ("getBytes".equals(method.getName())) return raw;
            if ("getObject".equals(method.getName())) return "delegated";
            if ("unwrap".equals(method.getName())) return proxy;
            throw new SQLException("Unexpected result method");
        });
    }
}
