import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.IndexMeta;
import org.apache.seata.sqlparser.struct.IndexType;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 执行固定原TableMeta类的19原案例观察与真实HashMap/异常边界，不修改原类或原JUnit。 */
public final class TableMetaOracle {
    private static void row(PrintWriter out,String method,Object... values){
        List<String> fields=new ArrayList<>();for(Object value:values){fields.add(String.valueOf(value));}
        out.println("org.apache.seata.sqlparser.struct.TableMetaTest#"+method+"\t"+HexFormat.of().formatHex(String.join("\u001f",fields).getBytes(StandardCharsets.UTF_8)));
    }
    private static void edge(PrintWriter out,String key,Object... values){
        List<String> fields=new ArrayList<>();for(Object value:values){fields.add(String.valueOf(value));}out.println(key+"\t"+String.join("\t",fields));
    }
    private static String units(String value){
        if(Objects.isNull(value)){return "null";}char[] digits="0123456789abcdef".toCharArray();StringBuilder out=new StringBuilder();
        for(char unit:value.toCharArray()){for(int shift:new int[]{12,8,4,0}){out.append(digits[(unit>>>shift)&15]);}}return out.toString();
    }
    private static TableMeta[] fixture(){
        TableMeta table=new TableMeta(),other=new TableMeta();table.setTableName("tableName");other.setTableName("tableName");
        ColumnMeta first=new ColumnMeta();first.setColumnName("col1");first.setOnUpdate(true);
        ColumnMeta second=new ColumnMeta();second.setColumnName("col2");
        IndexMeta primary=new IndexMeta();primary.setIndextype(IndexType.PRIMARY);primary.setValues(Arrays.asList(first,second));
        for(TableMeta current:new TableMeta[]{table,other}){current.getAllColumns().put("col1",first);current.getAllColumns().put("col2",second);current.getAllIndexes().put("primary",primary);}
        return new TableMeta[]{table,other};
    }
    private static void source(PrintWriter out){
        TableMeta table=fixture()[0];row(out,"testSetGetTableName",table.getTableName());
        table=fixture()[0];table.setCaseSensitive(true);boolean first=table.isCaseSensitive();table.setCaseSensitive(false);row(out,"testSetGetCaseSensitive",first,table.isCaseSensitive());
        table=fixture()[0];ColumnMeta column=new ColumnMeta();table.getAllColumns().put("col1",column);row(out,"testGetColumnMeta",column.equals(table.getColumnMeta("col1")));
        table=fixture()[0];table.getAllColumns().put("col1",new ColumnMeta());table.getAllColumns().put("col2",new ColumnMeta());Map<String,ColumnMeta> columns=table.getAllColumns();
        row(out,"testGetAllColumns",columns.size(),columns.containsKey("col1"),columns.containsKey("col2"));
        table=fixture()[0];Map<String,IndexMeta> indexes=table.getAllIndexes();row(out,"testGetAllIndexes",indexes.size(),indexes.containsKey("primary"));
        table=fixture()[0];Map<String,ColumnMeta> pk=table.getPrimaryKeyMap();row(out,"testGetPrimaryKeyMap",pk.size(),pk.containsKey("col1"),pk.containsKey("col2"));
        table=fixture()[0];table.getAllIndexes().clear();try{table.getPrimaryKeyMap();throw new AssertionError("Missing expected failure");}catch(org.apache.seata.common.exception.NotSupportYetException error){row(out,"testGetPrimaryKeyMapNoPrimaryKey",error.getClass().getName(),error.getMessage());}
        table=fixture()[0];table.getColumnMeta("col2".trim()).setColumnName("CoL2");Set<String> names=table.getCaseInsensitivePKs();row(out,"testGetCaseInsensitivePKs",names.size(),names.contains("col1"),names.contains("CoL2"));
        table=fixture()[0];table.getAllIndexes().clear();try{table.getCaseInsensitivePKs();throw new AssertionError("Missing expected failure");}catch(org.apache.seata.common.exception.NotSupportYetException error){row(out,"testGetCaseInsensitivePKsNoPrimaryKey",error.getClass().getName(),error.getMessage());}
        table=fixture()[0];List<String> keys=table.getPrimaryKeyOnlyName();row(out,"testGetPrimaryKeyOnlyName",keys.size(),keys.contains("col1"),keys.contains("col2"));
        table=fixture()[0];List<String> update=table.getOnUpdateColumnsOnlyName();List<String> expected=Arrays.asList("col1");row(out,"testGetOnUpdateColumnsOnlyName",update.size(),update.containsAll(expected));
        table=fixture()[0];table.getAllColumns().values().forEach(col->col.setOnUpdate(false));row(out,"testGetOnUpdateColumnsOnlyNameNoUpdates",table.getOnUpdateColumnsOnlyName().isEmpty());
        row(out,"testContainsPKWithNullList",fixture()[0].containsPK(null));
        row(out,"testContainsPKWithNoPrimaryKey",fixture()[0].containsPK(Arrays.asList("col3","col4")));
        row(out,"testContainsPKWithExactMatch",fixture()[0].containsPK(Arrays.asList("col1","col2")));
        row(out,"testContainsPKWithCaseInsensitiveMatch",fixture()[0].containsPK(Arrays.asList("COL1","COL2")));
        row(out,"testContainsPKWithNoMatch",fixture()[0].containsPK(Arrays.asList("other")));
        TableMeta[] pair=fixture();boolean same=pair[0].equals(pair[1]);pair[1].setTableName("different_table");row(out,"testEquals",same,pair[0].equals(pair[1]));
        pair=fixture();same=pair[0].hashCode()==pair[1].hashCode();pair[1].setTableName("different_table");row(out,"testHashCode",same,pair[0].hashCode()!=pair[1].hashCode());
    }
    private static String names(Iterable<String> names){List<String> result=new ArrayList<>();for(String name:names){result.add(units(name));}return String.join(";",result);}
    private static TableMeta primary(List<String> names){
        TableMeta table=new TableMeta();List<ColumnMeta> columns=new ArrayList<>();int i=0;
        for(String name:names){ColumnMeta col=new ColumnMeta();col.setColumnName(name);col.setDataType(++i);columns.add(col);}
        IndexMeta index=new IndexMeta();index.setIndextype(IndexType.PRIMARY);index.setValues(columns);table.getAllIndexes().put("primary",index);return table;
    }
    private static int spread(String key){int h=Objects.isNull(key)?0:key.hashCode();return h^(h>>>16);}
    private static void hashOrders(PrintWriter out){
        List<List<String>> groups=new ArrayList<>();groups.add(Arrays.asList("col1","col2","COL1","",null));
        List<String> zeros=new ArrayList<>();zeros.add(null);for(int i=0;i<=32;i++){zeros.add("\u0000".repeat(i));}groups.add(zeros);
        List<String> collisions=new ArrayList<>();for(int i=0;i<64;i++){StringBuilder key=new StringBuilder();for(int b=0;b<6;b++){key.append((i&(1<<b))==0?"Aa":"BB");}collisions.add(key.toString());}
        collisions.addAll(new ArrayList<>(collisions.subList(0,8)));groups.add(collisions);
        List<String> split=new ArrayList<>();int low=0,high=0;for(int i=0;low<20||high<20;i++){
            String key="key-"+i;int hash=spread(key);if((hash&63)==0){if((hash&64)==0&&low<20){split.add(key);low++;}else if((hash&64)!=0&&high<20){split.add(key);high++;}}
        }for(int i=0;i<120;i++){split.add("filler-"+i);}groups.add(split);
        groups.add(Arrays.asList("\ud800","\udc00","\ud800\udc00","\u0130","I","i","\u0131","Σ","ς","σ","",null));
        for(int group=0;group<groups.size();group++){List<String> values=groups.get(group);
            for(int n=1;n<=values.size();n++){
                List<String> prefix=values.subList(0,n);Map<String,ColumnMeta> map=primary(prefix).getPrimaryKeyMap();List<String> data=new ArrayList<>();
                for(Map.Entry<String,ColumnMeta> entry:map.entrySet()){data.add(Integer.toString(entry.getValue().getDataType()));}
                edge(out,"hash/"+group+"/"+n,names(prefix),names(map.keySet()),String.join(";",data),map.size());
            }
        }
    }
    private static final class MyNpe extends java.lang.NullPointerException{MyNpe(String message){super(message);}}
    private static final class MyCce extends ClassCastException{MyCce(String message){super(message);}}
    private static final class MyCme extends java.util.ConcurrentModificationException{MyCme(String message){super(message);}}
    private static final class NullPointerException extends IllegalStateException{NullPointerException(String message){super(message);}}
    private static final class ThrowingEquals{
        private final RuntimeException error;ThrowingEquals(RuntimeException error){this.error=error;}
        @Override public boolean equals(Object other){throw error;}
        @Override public int hashCode(){return 7;}
    }
    private static void hierarchy(PrintWriter out){
        String message="\ud800\u0000\udc00";
        RuntimeException[] errors={new MyNpe(message),new MyCce(message),new MyCme(message),new NullPointerException(message)};
        for(int i=0;i<errors.length;i++){
            RuntimeException failure=errors[i];TableMeta mine=new TableMeta(),theirs=new TableMeta();ColumnMeta left=new ColumnMeta(),right=new ColumnMeta();
            left.setCharOctetLength(new ThrowingEquals(failure));right.setCharOctetLength(1);mine.getAllColumns().put("c",left);theirs.getAllColumns().put("c",right);
            List<String> parents=new ArrayList<>();for(Class<?> type=failure.getClass().getSuperclass();Objects.nonNull(type);type=type.getSuperclass()){parents.add(type.getName());}
            String concrete="";String raw="";boolean identity=false;try{right.equals(left);}catch(RuntimeException actual){concrete=actual.getClass().getName();raw=units(actual.getMessage());identity=actual==failure;}
            String equality;try{equality="B:"+mine.equals(theirs);}catch(RuntimeException actual){equality="E:"+actual.getClass().getName();}
            edge(out,"hierarchy/"+i,failure.getClass().getName(),String.join(";",parents),concrete,raw,identity,equality);
        }
    }
    private static void edges(PrintWriter out){
        for(IndexType type:IndexType.values()){edge(out,"enum-identity/"+type.name(),System.identityHashCode(type));}
        TableMeta first=new TableMeta(),second=new TableMeta();edge(out,"table-default",units(first.getTableName()),units(first.getOriginalTableName()),first.isCaseSensitive(),first.getAllColumns().size(),first.getAllIndexes().size(),first.hashCode());
        first.setOriginalTableName("one");second.setOriginalTableName("two");boolean same=first.equals(second);boolean hash=first.hashCode()==second.hashCode();second.setCaseSensitive(true);
        edge(out,"ignored-original-and-case-hash",same,hash,first.equals(second),first.hashCode(),second.hashCode());
        for(String language:new String[]{"en","tr","az","lt","th"}){
            Locale.setDefault(Locale.forLanguageTag(language));TableMeta table=new TableMeta();String[] keys={"I","i","\u0130","\u0131","AภาษาΣ"};
            for(int i=0;i<keys.length;i++){ColumnMeta column=new ColumnMeta();column.setDataType(i+1);table.getAllColumns().put(keys[i],column);}
            List<String> state=new ArrayList<>();for(Map.Entry<String,ColumnMeta> entry:table.getAllColumns().entrySet()){state.add(units(entry.getKey())+":"+entry.getValue().getDataType());}
            edge(out,"map-language/"+language,String.join(";",state),table.getAllColumns().size());
        }
        hierarchy(out);
    }
    public static void main(String[] args)throws Exception{
        Path out=Path.of(args[0]);Locale original=Locale.getDefault();
        try(PrintWriter source=new PrintWriter(Files.newBufferedWriter(out.resolve("table-java.tsv"),StandardCharsets.UTF_8));
            PrintWriter hashes=new PrintWriter(Files.newBufferedWriter(out.resolve("primary-hash-java.tsv"),StandardCharsets.UTF_8));
            PrintWriter boundaries=new PrintWriter(Files.newBufferedWriter(out.resolve("table-edges-java.tsv"),StandardCharsets.UTF_8));
            PrintWriter origin=new PrintWriter(Files.newBufferedWriter(out.resolve("table-origin.tsv"),StandardCharsets.UTF_8))){
            source(source);hashOrders(hashes);edges(boundaries);
            origin.println("org.apache.seata.sqlparser.struct.TableMeta\t"+TableMeta.class.getProtectionDomain().getCodeSource().getLocation());origin.println("default_language\t"+original.getLanguage());
        }finally{Locale.setDefault(original);}
    }
}
