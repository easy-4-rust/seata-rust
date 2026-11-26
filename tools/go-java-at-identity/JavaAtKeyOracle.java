// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
import com.fasterxml.jackson.databind.ObjectMapper;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.sql.Types;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Collections;
import java.util.Random;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.rm.datasource.exec.BaseTransactionalExecutor;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.KeyType;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.IndexMeta;
import org.apache.seata.sqlparser.struct.IndexType;
import org.apache.seata.sqlparser.struct.TableMeta;
import org.mockito.Mockito;
import org.springframework.mock.env.MockEnvironment;

/** 原Java真实TableMeta与BaseTransactionalExecutor锁键导出，仅作固定版本差分依据。 */
public final class JavaAtKeyOracle {
    /** 参数为唯一D/E输出文件；返回原对象实际主键次序和实际锁键，不修改数据库。 */
    public static void main(String[] args) throws Exception {
        Path output=Path.of(args[0]).toAbsolutePath().normalize();
        if (!(output.toString().startsWith("D:\\")||output.toString().startsWith("E:\\"))) throw new IllegalArgumentException("D/E only");
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT,new MockEnvironment().withProperty("seata.config.type","file").withProperty("seata.config.file.name","file"));
        List<List<String>> cases=new ArrayList<>();
        cases.add(Arrays.asList("id"));cases.add(Arrays.asList("z","a"));cases.add(Arrays.asList("id","tenant"));cases.add(Arrays.asList("BB","Aa"));cases.add(Arrays.asList("订单","租户","id"));
        List<String> regular=new ArrayList<>();for(int i=0;i<16;i++)regular.add("column"+i);cases.add(regular);
        for(int length:Arrays.asList(8,9,10,11,12,16)){
            List<String> colliding=new ArrayList<>();for(int i=0;i<length;i++){StringBuilder name=new StringBuilder();for(int bit=0;bit<4;bit++)name.append((i&(1<<bit))==0?"Aa":"BB");colliding.add(name.toString());}cases.add(colliding);
        }
        Random random=new Random(7312026L);List<String> mixedBucket=new ArrayList<>();boolean positive=false,negative=false;
        while(mixedBucket.size()<16){String key=Long.toHexString(random.nextLong());int hash=key.hashCode()^(key.hashCode()>>>16);if((hash&63)==3){mixedBucket.add(key);positive|=hash>=0;negative|=hash<0;}}
        if(!positive||!negative)throw new IllegalStateException("Signed hash control missing");
        cases.add(new ArrayList<>(mixedBucket.subList(0,11)));cases.add(new ArrayList<>(mixedBucket));
        for(int iteration=0;iteration<100;iteration++){List<String> names=new ArrayList<>(mixedBucket);Collections.shuffle(names,random);cases.add(names);}
        for(int iteration=0;iteration<100;iteration++){List<String> names=new ArrayList<>();for(int i=0;i<1+iteration%16;i++)names.add("column_"+Long.toHexString(random.nextLong()));cases.add(names);}
        Method build=BaseTransactionalExecutor.class.getDeclaredMethod("buildLockKey",TableRecords.class);build.setAccessible(true);
        Object executor=Mockito.mock(BaseTransactionalExecutor.class,Mockito.CALLS_REAL_METHODS);List<Map<String,Object>> results=new ArrayList<>();
        for(List<String> names:cases){
            TableMeta meta=new TableMeta();meta.setTableName("AccountLedger");IndexMeta index=new IndexMeta();index.setIndextype(IndexType.PRIMARY);List<ColumnMeta> columns=new ArrayList<>();Row row=new Row();
            for(int i=0;i<names.size();i++){String name=names.get(i);ColumnMeta column=new ColumnMeta();column.setColumnName(name);column.setDataType(Types.BIGINT);columns.add(column);meta.getAllColumns().put(name,column);Field field=new Field();field.setName(name);field.setType(Types.BIGINT);field.setKeyType(KeyType.PRIMARY_KEY);field.setValue((long)i+1);row.add(field);}
            index.setValues(columns);meta.getAllIndexes().put("PRIMARY",index);TableRecords records=new TableRecords(meta);records.getRows().add(row);
            Map<String,Object> result=new LinkedHashMap<>();result.put("input",names);result.put("order",meta.getPrimaryKeyOnlyName());result.put("key",build.invoke(executor,records));results.add(result);
        }
        Files.write(output,new ObjectMapper().writerWithDefaultPrettyPrinter().writeValueAsBytes(results),StandardOpenOption.CREATE_NEW);
        System.out.println("JAVA_AT_KEY_ORACLE cases="+results.size());
    }
}
