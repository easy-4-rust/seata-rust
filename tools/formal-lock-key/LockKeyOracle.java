package org.apache.seata.rm.datasource;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Statement;
import java.sql.Types;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.common.util.ArrayUtils;
import org.apache.seata.rm.datasource.exec.BaseTransactionalExecutor;
import org.apache.seata.rm.datasource.exec.StatementCallback;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.sqlparser.SQLRecognizer;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.IndexMeta;
import org.apache.seata.sqlparser.struct.IndexType;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 自有固定锁键探针，实际调用固定原源方法；不建立SQL连接或登记分支。 */
public final class LockKeyOracle extends BaseTransactionalExecutor<Object,Statement> {
    private LockKeyOracle() {
        super((StatementProxy<Statement>)null,(StatementCallback<Object,Statement>)null,(SQLRecognizer)null);
    }
    @Override protected Object doExecute(Object... args) {
        throw new UnsupportedOperationException("Test oracle never executes business SQL");
    }
    public static String key(TableRecords records) {return new LockKeyOracle().buildLockKey(records);}
    public static String combined(List<String> keys) {
        ConnectionContext context=new ConnectionContext();
        for(String key:keys)if(Objects.nonNull(key))context.appendLockKey(key);
        return context.buildLockKeys();
    }
    public static void main(String[] args) throws Exception {
        ObjectMapper mapper=new ObjectMapper();LockKeyOracle oracle=new LockKeyOracle();
        List<?> cases=mapper.readValue(Files.readAllBytes(Path.of(args[0])),List.class);List<Object> result=new ArrayList<>();
        for(Object raw:cases) {
            Map<?,?> item=(Map<?,?>)raw;List<?> names=(List<?>)item.get("names");
            TableMeta table=new TableMeta();table.setTableName((String)item.get("table"));
            List<ColumnMeta> columns=new ArrayList<>();
            for(Object name:names) {ColumnMeta column=new ColumnMeta();column.setColumnName((String)name);columns.add(column);table.getAllColumns().put((String)name,column);}
            IndexMeta index=new IndexMeta();index.setIndextype(IndexType.PRIMARY);index.setValues(columns);table.getAllIndexes().put("PRIMARY",index);
            TableRecords records=new TableRecords(table);
            for(Object rowRaw:(List<?>)item.get("rows")) {
                List<?> values=(List<?>)rowRaw;Row row=new Row();
                for(int position=values.size()-1;position>=0;position--) {
                    Object value=values.get(position);
                    if(value instanceof Map<?,?>) {
                        Map<?,?> tagged=(Map<?,?>)value;String kind=(String)tagged.get("kind");String text=(String)tagged.get("text");
                        if("decimal".equals(kind))value=new java.math.BigDecimal(text);
                        else if("bigint".equals(kind))value=new java.math.BigInteger(text);
                        else if("binary".equals(kind))value=java.util.HexFormat.of().parseHex(text);
                        else if("boolean".equals(kind))value=Boolean.valueOf(text);
                        else throw new IllegalArgumentException("Owned oracle value kind");
                    }
                    row.add(new Field((String)names.get(position),Types.OTHER,value));
                }
                records.add(row);
            }
            String key=null;boolean rejected=false;
            try {key=oracle.buildLockKey(records);}catch(IllegalArgumentException expected){rejected=true;}
            if(item.containsKey("expected")) {
                if(!Objects.equals(item.get("expected"),key)||rejected)throw new AssertionError("Original source fixture differs: "+item.get("case"));
            }
            if(item.containsKey("candidate")&&!Objects.equals(item.get("candidate"),key))throw new AssertionError("Original Java/Rust lock-key differential failed: "+item.get("case"));
            if(item.containsKey("candidate_rejected")&&!Objects.equals(item.get("candidate_rejected"),rejected))throw new AssertionError("Original Java/Rust rejection differential failed: "+item.get("case"));
            ConnectionContext context=new ConnectionContext();
            if(Objects.nonNull(key)) {context.appendLockKey(key);context.appendLockKey(key);}
            if(!Objects.equals(key,context.buildLockKeys()))throw new AssertionError("Original complete-key de-duplication differs");
            Map<String,Object> row=new LinkedHashMap<>();row.put("case",item.get("case"));row.put("java_primary_names",table.getPrimaryKeyOnlyName());
            row.put("lock_key",key);row.put("rejected",rejected);result.add(row);
        }
        Path target=Path.of(args[1]);Files.createDirectories(target.getParent());Files.write(target,mapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(result));
        System.out.println("original_lock_key_cases="+result.size()+" binary_example="+ArrayUtils.toString(new byte[]{0,-1}));
        System.exit(0);
    }
}
