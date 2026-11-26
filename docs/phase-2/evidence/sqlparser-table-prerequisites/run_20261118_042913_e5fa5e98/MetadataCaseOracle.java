import java.io.DataOutputStream;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.TreeSet;
import org.apache.seata.sqlparser.struct.ColumnMeta;
import org.apache.seata.sqlparser.struct.IndexMeta;
import org.apache.seata.sqlparser.struct.IndexType;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 固定JDK17字符映射输入和原TableMeta主键集合的实际行为；不修改原生产类。 */
public final class MetadataCaseOracle {
    private static String units(String text) {
        if (Objects.isNull(text)) { return "null"; }
        StringBuilder out = new StringBuilder();
        for (int i=0; i<text.length(); i++) { out.append(String.format("%04x", (int)text.charAt(i))); }
        return out.toString();
    }
    private static String names(Set<String> values) {
        List<String> encoded = new ArrayList<>();
        for (String name : values) { encoded.add(units(name)); }
        return String.join(";", encoded);
    }
    private static void pair(PrintWriter out, String left, String right) {
        out.println(units(left)+"\t"+units(right)+"\t"+String.CASE_INSENSITIVE_ORDER.compare(left, right));
    }
    private static String scalar(int cp) { return new String(Character.toChars(cp)); }
    private static String raw(int... units) {
        char[] chars = new char[units.length];
        for (int i=0; i<units.length; i++) { chars[i]=(char)units[i]; }
        return new String(chars);
    }
    private static String contains(Set<String> set, String name) {
        try { return Boolean.toString(set.contains(name)); }
        catch (RuntimeException error) { return error.getClass().getName(); }
    }
    private static String mutation(Set<String> set, int operation) {
        try {
            switch (operation) {
                case 0: set.add(null); break;
                case 1: set.remove("missing"); break;
                case 2: set.clear(); break;
                case 3: set.addAll(Collections.emptyList()); break;
                default: throw new IllegalArgumentException();
            }
            return "NO_EXCEPTION";
        } catch (RuntimeException error) { return error.getClass().getName(); }
    }
    private static void set(PrintWriter out, int id, String[] input) {
        // 由原TableMeta方法创建结果，确保不是观察器自己实现主键筛选或排序。
        TableMeta table = new TableMeta();
        List<ColumnMeta> columns = new ArrayList<>();
        for (String name : input) { ColumnMeta col = new ColumnMeta(); col.setColumnName(name); columns.add(col); }
        IndexMeta primary = new IndexMeta(); primary.setIndextype(IndexType.PRIMARY); primary.setValues(columns);
        table.getAllIndexes().put("primary", primary);
        List<String> encoded = new ArrayList<>(); for (String name : input) { encoded.add(units(name)); }
        String prefix = "set/"+id+"\t"+String.join(";", encoded);
        try {
            Set<String> result = table.getCaseInsensitivePKs();
            List<String> probes = new ArrayList<>();
            for (String name : input) { probes.add(contains(result, name)); }
            probes.add(contains(result, null)); probes.add(contains(result, "missing"));
            List<String> mutations = new ArrayList<>(); for(int i=0;i<4;i++){mutations.add(mutation(result,i));}
            out.println(prefix+"\tOK\t"+names(result)+"\t"+result.size()+"\t"+result.hashCode()
                +"\t"+String.join(";", probes)+"\t"+String.join(";", mutations));
        } catch (RuntimeException error) { out.println(prefix+"\t"+error.getClass().getName()); }
    }
    public static void main(String[] args) throws Exception {
        Path output=Path.of(args[0]); Files.createDirectories(output);
        int changed=0;
        try (DataOutputStream data=new DataOutputStream(Files.newOutputStream(output.resolve("java17-character-case-input.bin")))) {
            for(int cp=0;cp<=Character.MAX_CODE_POINT;cp++) {
                int upper=Character.toUpperCase(cp); int lower=Character.toLowerCase(cp);
                if(upper!=cp || lower!=cp){data.writeInt(cp);data.writeInt(upper);data.writeInt(lower);changed++;}
            }
        }
        List<String> examples=new ArrayList<>(Arrays.asList("", "a", "A", "aZ", "Az", "I", "i", "\u0130", "\u0131",
            "\u00df", "SS", "\u017f", "S", "\u03a3", "\u03c3", "\u03c2", "\u212a", "K", "\u212b", "\u00e5",
            "\u10d0", "\u1c90", "\ua7d0", "\ua7d1", "a\u0000", "\u0000a", raw(0xd800), raw(0xdc00),
            raw(0xd801,0xdc00), raw(0xd801,0xdc28), raw(0xd801,0xdc00,0xdc00), raw(0xd801,0xdc28,0xd800),
            raw(0xd800,0xd801,0xdc00), raw(0xd800,0xd801,0xdc28), raw(0xd801,0xdc00,0x61), raw(0xd801,0xdc28,0x41)));
        int pairs=0;
        try (PrintWriter out=new PrintWriter(Files.newBufferedWriter(output.resolve("java17-comparisons.tsv"),StandardCharsets.UTF_8))) {
            for(String left:examples){for(String right:examples){pair(out,left,right);pairs++;}}
            // 全部发生大小写变化的码点都经过真实比较；映射表本身只是运行时输入。
            for(int cp=0;cp<=Character.MAX_CODE_POINT;cp++) {
                int upper=Character.toUpperCase(cp); int lower=Character.toLowerCase(cp);
                if(upper!=cp || lower!=cp) {
                    String text=scalar(cp); String u=scalar(upper); String l=scalar(lower);
                    pair(out,text,u); pair(out,text,l); pair(out,text+"A",l+"a"); pair(out,l+"z",text+"A"); pairs+=4;
                }
            }
            // 固定LCG生成UTF16原始序列，包含错配代理码元及不同字符串长度。
            long state=0x51ea7aL;
            for(int n=0;n<10000;n++) {
                StringBuilder left=new StringBuilder(),right=new StringBuilder();
                state=(state*1664525+1013904223)&0xffffffffL; int ll=(int)(state%8);
                state=(state*1664525+1013904223)&0xffffffffL; int rl=(int)(state%8);
                for(int i=0;i<ll;i++){state=(state*1664525+1013904223)&0xffffffffL;left.append((char)(state>>>8));}
                for(int i=0;i<rl;i++){state=(state*1664525+1013904223)&0xffffffffL;right.append((char)(state>>>8));}
                pair(out,left.toString(),right.toString());pairs++;
            }
        }
        List<String[]> sets=Arrays.asList(new String[]{"col1","CoL2"},new String[]{"a","A","b","B"},
            new String[]{"I","i","\u0130","\u0131"},new String[]{"\u03a3","\u03c3","\u03c2"},
            new String[]{"\u017f","S","s"},new String[]{"\u00df","SS","ss"},
            new String[]{"\u10d0","\u1c90"},new String[]{"\ua7d0","\ua7d1"},
            new String[]{raw(0xd801,0xdc00),raw(0xd801,0xdc28),"a"},
            new String[]{raw(0xd800),raw(0xdc00),"\u0000",""},
            new String[]{null},new String[]{"x",null},new String[]{"z","a","B","A","b"});
        try(PrintWriter out=new PrintWriter(Files.newBufferedWriter(output.resolve("table-primary-sets.tsv"),StandardCharsets.UTF_8))) {
            for(int i=0;i<sets.size();i++){set(out,i,sets.get(i));}
            Set<String> empty=Collections.unmodifiableSet(new TreeSet<>(String.CASE_INSENSITIVE_ORDER));
            out.println("empty\t"+contains(empty,null)+"\t"+mutation(empty,0)+"\t"+mutation(empty,1)+"\t"+mutation(empty,2)+"\t"+mutation(empty,3));
            Set<String> upper=new TreeSet<>(String.CASE_INSENSITIVE_ORDER);upper.add("A");
            Set<String> lower=new TreeSet<>(String.CASE_INSENSITIVE_ORDER);lower.add("a");
            out.println("set-equality\t"+upper.equals(lower)+"\t"+lower.equals(upper)+"\t"+upper.hashCode()+"\t"+lower.hashCode());
        }
        try(PrintWriter out=new PrintWriter(Files.newBufferedWriter(output.resolve("case-origin.tsv"),StandardCharsets.UTF_8))) {
            out.println("org.apache.seata.sqlparser.struct.TableMeta\t"+TableMeta.class.getProtectionDomain().getCodeSource().getLocation());
            out.println("JDK\t"+System.getProperty("java.version"));
            out.println("character_mapping_inputs\t"+changed);out.println("computed_comparisons\t"+pairs);
        }
    }
}
