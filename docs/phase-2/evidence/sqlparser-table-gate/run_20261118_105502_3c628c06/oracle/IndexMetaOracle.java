import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.List;
import org.apache.seata.sqlparser.SQLParsingException;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.IndexMeta;
import org.apache.seata.sqlparser.struct.IndexType;

/** 原IndexMeta16案例及JDK列表交互的实际观察器；不更改原类、断言或集合实现。 */
public final class IndexMetaOracle {
    private static void row(PrintWriter out, String method, Object... values) {
        List<String> fields = new ArrayList<>(); for (Object value : values) { fields.add(String.valueOf(value)); }
        out.println("org.apache.seata.sqlparser.struct.IndexMetaTest#"+method+"\t"
            +HexFormat.of().formatHex(String.join("\u001f", fields).getBytes(StandardCharsets.UTF_8)));
    }
    private static void edge(PrintWriter out, String key, Object... values) {
        List<String> fields = new ArrayList<>(); for (Object value : values) { fields.add(String.valueOf(value)); }
        out.println(key+"\t"+String.join("\t", fields));
    }
    private static String units(String text) {
        if (java.util.Objects.isNull(text)) { return "null"; }
        StringBuilder out = new StringBuilder(); for (int i=0;i<text.length();i++) { out.append(String.format("%04x", (int)text.charAt(i))); }
        return out.toString();
    }
    private static ColumnMeta column(String name) { ColumnMeta value=new ColumnMeta(); value.setColumnName(name); return value; }
    private static List<ColumnMeta> list(String mode) {
        List<ColumnMeta> values=Arrays.asList(column("first"),column("second"));
        if ("F".equals(mode)) { return values; }
        if ("A".equals(mode)) { return new ArrayList<>(values); }
        List<ColumnMeta> grown=new ArrayList<>(); for(ColumnMeta value:values){grown.add(value);} return grown;
    }
    private static String names(List<ColumnMeta> values) {
        List<String> names=new ArrayList<>(); for (ColumnMeta value:values) { names.add(java.util.Objects.isNull(value)?"null":value.getColumnName()); }
        return String.join(",",names);
    }
    private static final class Probe {
        private final Runnable action;
        private final int[] count;
        private boolean fired;
        private Probe(Runnable action,int[] count) { this.action=action;this.count=count; }
        private void fire() { count[0]++; if(!fired){fired=true;action.run();} }
        @Override public boolean equals(Object other) { fire(); return other instanceof Probe; }
        @Override public int hashCode() { return 11; }
        @Override public String toString() { fire();return "callback"; }
    }
    private static Runnable action(String action,List<ColumnMeta> left,List<ColumnMeta> right) {
        return () -> {
            switch(action) {
                case "clear-left": left.clear(); break;
                case "clear-right": right.clear(); break;
                case "clear-both": left.clear();right.clear();break;
                case "set-left": left.set(1,column("changed"));break;
                case "set-right": right.set(1,column("changed"));break;
                case "null-left": left.set(1,null);break;
                case "null-right": right.set(1,null);break;
                case "remove-left": left.remove(1);break;
                case "remove-right": right.remove(1);break;
                case "grow-left-set": left.add(column("extra"));left.set(1,column("changed"));break;
                case "grow-right-set": right.add(column("extra"));right.set(1,column("changed"));break;
                default: break;
            }
        };
    }
    private static IndexMeta index(boolean second,boolean different) {
        IndexMeta value=new IndexMeta(); List<ColumnMeta> cols=new ArrayList<>(); for(int i=0;i<(different?3:2);i++){cols.add(new ColumnMeta());}
        value.setValues(Arrays.asList(cols.toArray(new ColumnMeta[0]))); value.setNonUnique(!(second&&different));
        value.setIndexQualifier(second&&different?"differentQualifier":"qualifier");value.setIndexName(second&&different?"differentIndexName":"indexName");
        value.setType((short)(second&&different?2:1));value.setIndextype(different?(second?IndexType.FULL_TEXT:IndexType.UNIQUE):IndexType.PRIMARY);
        value.setAscOrDesc(second&&different?"D":"A");value.setOrdinalPosition(second&&different?2:1);return value;
    }
    private static ColumnMeta fullColumn(int variant) {
        ColumnMeta value=new ColumnMeta();String suffix=variant==0?"":variant==1?"1":"2";
        value.setTableCat("tableCat"+suffix);value.setTableSchemaName("schemaName"+suffix);value.setTableName("tableName"+suffix);value.setColumnName("columnName"+suffix);
        value.setDataType(variant==2?2:1);value.setDataTypeName("dataTypeName"+suffix);value.setColumnSize(variant==2?200:100);
        value.setDecimalDigits(variant==2?20:10);value.setNumPrecRadix(variant==2?20:10);value.setNullAble(variant==2?0:1);
        value.setRemarks("remarks"+suffix);value.setColumnDef("columnDef"+suffix);value.setSqlDataType(variant==2?2:1);value.setSqlDatetimeSub(variant==2?2:1);
        value.setCharOctetLength(variant==2?200:100);value.setOrdinalPosition(variant==2?2:1);value.setIsNullAble(variant==2?"NO":"YES");
        value.setIsAutoincrement(variant==2?"NO":"YES");value.setOnUpdate(variant!=2);value.setCaseSensitive(variant!=2);return value;
    }
    private static void sourceCases(PrintWriter out) {
        IndexMeta value=new IndexMeta();List<ColumnMeta> cols=Arrays.asList(new ColumnMeta(),new ColumnMeta());value.setValues(cols);row(out,"testGetSetValues",cols.equals(value.getValues()));
        value=new IndexMeta();value.setNonUnique(true);boolean yes=value.isNonUnique();value.setNonUnique(false);row(out,"testIsSetNonUnique",yes,value.isNonUnique());
        value=new IndexMeta();value.setIndexQualifier("qualifier");row(out,"testGetSetIndexQualifier",value.getIndexQualifier());
        value=new IndexMeta();value.setIndexName("indexName");row(out,"testGetSetIndexName",value.getIndexName());
        value=new IndexMeta();value.setType((short)1);row(out,"testGetSetType",value.getType());
        value=new IndexMeta();value.setAscOrDesc("A");row(out,"testGetSetAscOrDesc",value.getAscOrDesc());
        value=new IndexMeta();value.setCardinality(100L);row(out,"testGetSetCardinality",value.getCardinality());
        value=new IndexMeta();value.setOrdinalPosition(1);row(out,"testGetSetOrdinalPosition",value.getOrdinalPosition());
        value=new IndexMeta();value.setIndextype(IndexType.NORMAL);row(out,"testGetSetIndextype",value.getIndextype());
        value=new IndexMeta();row(out,"testEqualsSameObject",value.equals(value));row(out,"testEqualsNullObject",value.equals(null));
        row(out,"testEqualsDifferentType",value.equals("I am not an IndexMeta"));
        row(out,"testEqualsIdenticalObjects",index(false,false).equals(index(true,false)));
        row(out,"testEqualsDifferentObjects",index(false,true).equals(index(true,true)));
        row(out,"testHashCodeConsistency",fullColumn(0).hashCode()==fullColumn(0).hashCode());
        row(out,"testHashCodeDifference",fullColumn(1).hashCode()!=fullColumn(2).hashCode());
    }
    private static void extraCases(PrintWriter out) {
        for(IndexType type:IndexType.values()){edge(out,"enum-identity/"+type,type.hashCode());}
        IndexMeta a=new IndexMeta(),b=new IndexMeta();
        edge(out,"index-default",a.hashCode(),units(a.toString()));
        try{a.equals(b);throw new AssertionError("Missing null-indextype failure");}catch(NullPointerException e){edge(out,"index-null-type",e.getClass().getName());}
        b.setValues(null);edge(out,"index-null-list-shortcut",a.equals(b));a.setValues(null);a.setIndextype(IndexType.PRIMARY);b.setIndextype(IndexType.PRIMARY);
        a.setCardinality(Long.MIN_VALUE);b.setCardinality(Long.MAX_VALUE);edge(out,"index-ignored-cardinality",a.equals(b),a.hashCode()==b.hashCode());
        edge(out,"index-null-list-text",units(a.toString()));
        a.setType(Short.MIN_VALUE);a.setCardinality(Long.MIN_VALUE);edge(out,"index-wide-min",a.getType(),a.getCardinality());
        a.setType(Short.MAX_VALUE);a.setCardinality(Long.MAX_VALUE);edge(out,"index-wide-max",a.getType(),a.getCardinality());
        for(IndexType type:IndexType.values()){
            IndexMeta h=index(false,false);h.setIndextype(type);edge(out,"index-hash/"+type,h.hashCode());
        }
        String[] modes={"A","D","F"};String[] actions={"none","clear-left","clear-right","clear-both","set-left","set-right","null-left","null-right","remove-left","remove-right","grow-left-set","grow-right-set"};
        for(String lm:modes){for(String rm:modes){for(String op:actions){
            List<ColumnMeta> left=list(lm),right=list(rm);int[] probe={0};
            left.get(0).setCharOctetLength(new Probe(()->{},new int[1]));right.get(0).setCharOctetLength(new Probe(action(op,left,right),new int[1]));
            left.get(1).setCharOctetLength(new Probe(()->{},new int[1]));right.get(1).setCharOctetLength(new Probe(()->{},probe));
            String result;try{result="B:"+left.equals(right);}catch(Throwable e){result="E:"+e.getClass().getName();}
            edge(out,"eq/"+lm+"/"+rm+"/"+op,result,left.size(),right.size(),probe[0],names(left),names(right));
        }}}
        String[] textActions={"none","clear-left","remove-left","null-left","set-left","grow-left-set"};
        for(String mode:modes){for(String op:textActions){
            List<ColumnMeta> values=list(mode);int[] probe={0};
            values.get(0).setCharOctetLength(new Probe(action(op,values,values),new int[1]));values.get(1).setCharOctetLength(new Probe(()->{},probe));
            String result;try{result="S:"+units(values.toString());}catch(Throwable e){result="E:"+e.getClass().getName();}
            edge(out,"text/"+mode+"/"+op,result,values.size(),probe[0],names(values));
        }}
        RuntimeException original=new IllegalStateException(new String(new char[]{(char)0xd800,0,(char)0xdc00}));
        Object failing=new Object(){@Override public boolean equals(Object other){throw original;}@Override public int hashCode(){throw original;}@Override public String toString(){throw original;}};
        ColumnMeta good=new ColumnMeta();good.setCharOctetLength(1);ColumnMeta bad=new ColumnMeta();bad.setCharOctetLength(failing);
        for(int i=0;i<7;i++){
            try{
                switch(i){
                    case 0:good.equals(bad);break;case 1:bad.hashCode();break;case 2:bad.toString();break;
                    case 3:Arrays.asList(bad).toString();break;
                    case 4:IndexMeta x=new IndexMeta();x.setValues(Arrays.asList(bad));x.toString();break;
                    case 5:IndexMeta x1=new IndexMeta(),x2=new IndexMeta();x1.setValues(Arrays.asList(bad));x2.setValues(Arrays.asList(good));x1.equals(x2);break;
                    default:new SQLParsingException(new RuntimeException(){@Override public String toString(){throw original;}});break;
                }
                throw new AssertionError("Dynamic object failure swallowed");
            }catch(Throwable error){edge(out,"failure/"+i,error.getClass().getName(),units(error.getMessage()),error==original);}
        }
    }
    /** 参数为原16案例观察、额外场景和类来源文件；所有输出只写入调用方显式目录。 */
    public static void main(String[] args)throws Exception{
        try(PrintWriter out=new PrintWriter(Files.newBufferedWriter(Path.of(args[0]),StandardCharsets.UTF_8))){sourceCases(out);if(out.checkError()){throw new IllegalStateException("Incomplete original16 observations");}}
        try(PrintWriter out=new PrintWriter(Files.newBufferedWriter(Path.of(args[1]),StandardCharsets.UTF_8))){extraCases(out);if(out.checkError()){throw new IllegalStateException("Incomplete interaction observations");}}
        Files.writeString(Path.of(args[2]),IndexMeta.class.getName()+"\t"+IndexMeta.class.getProtectionDomain().getCodeSource().getLocation()+"\n",StandardCharsets.UTF_8);
    }
}
