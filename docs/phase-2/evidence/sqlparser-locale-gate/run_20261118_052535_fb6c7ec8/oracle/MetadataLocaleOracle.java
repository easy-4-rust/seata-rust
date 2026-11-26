import java.io.DataOutputStream;
import java.io.PrintWriter;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.text.BreakIterator;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.Objects;

/** 导出实际JDK17上下文大小写/分词结果及其版本化输入，不重新实现观察器中的算法。 */
public final class MetadataLocaleOracle {
    private static String units(String text){
        char[] digits="0123456789abcdef".toCharArray();StringBuilder out=new StringBuilder(text.length()*4);
        for(char unit:text.toCharArray()){for(int shift:new int[]{12,8,4,0}){out.append(digits[(unit>>>shift)&15]);}}
        return out.toString();
    }
    private static void chars(DataOutputStream out,String text)throws Exception{
        out.writeShort(text.length());for(char unit:text.toCharArray()){out.writeChar(unit);}
    }
    private static void shorts(DataOutputStream out,short[] values)throws Exception{
        out.writeInt(values.length);for(short value:values){out.writeShort(value);}
    }
    private static void ints(DataOutputStream out,int[] values)throws Exception{
        out.writeInt(values.length);for(int value:values){out.writeInt(value);}
    }
    private static void compact(DataOutputStream out,Object table,Object supplement)throws Exception{
        shorts(out,(short[])MetadataLocaleProbe.field(table,"indices"));
        byte[] values=(byte[])MetadataLocaleProbe.field(table,"values");out.writeInt(values.length);out.write(values);
        ints(out,(int[])MetadataLocaleProbe.field(supplement,"dataTable"));
    }
    private static String boundary(String text,Locale locale){
        BreakIterator word=BreakIterator.getWordInstance(locale);word.setText(text);StringBuilder result=new StringBuilder();
        for(int i=0;i<=text.length();i++){result.append(word.isBoundary(i)?'1':'0');}return result.toString();
    }
    public static void main(String[] args)throws Exception{
        Path output=Path.of(args[0]);Files.createDirectories(output);
        Method cased=Class.forName("java.lang.ConditionalSpecialCasing").getDeclaredMethod("isCased",int.class);cased.setAccessible(true);
        Method combining=Class.forName("sun.text.Normalizer").getMethod("getCombiningClass",int.class);combining.setAccessible(true);
        int records=0;
        try(DataOutputStream out=new DataOutputStream(Files.newOutputStream(output.resolve("jdk17-string-casing-input.bin")))){
            for(int cp=0;cp<=Character.MAX_CODE_POINT;cp++){
                String text=new String(Character.toChars(cp)),lower=text.toLowerCase(Locale.ROOT),upper=text.toUpperCase(Locale.ROOT);
                int cc=(Integer)combining.invoke(null,cp);boolean isCased=(Boolean)cased.invoke(null,cp);
                if(!text.equals(lower)||!text.equals(upper)||cc!=0||isCased){
                    out.writeInt(cp);out.writeByte(cc);out.writeBoolean(isCased);chars(out,lower);chars(out,upper);records++;
                }
            }
        }
        Files.write(output.resolve("jdk17-word-root-input.bin"),MetadataLocaleProbe.profile(BreakIterator.getWordInstance(Locale.ROOT)));
        BreakIterator thai=BreakIterator.getWordInstance(Locale.forLanguageTag("th"));
        Files.write(output.resolve("jdk17-word-th-input.bin"),MetadataLocaleProbe.profile(thai));
        Object dictionary=MetadataLocaleProbe.field(thai,"dictionary");
        try(DataOutputStream out=new DataOutputStream(Files.newOutputStream(output.resolve("jdk17-word-th-dictionary-input.bin")))){
            out.writeInt((Integer)MetadataLocaleProbe.field(dictionary,"numCols"));
            compact(out,MetadataLocaleProbe.field(dictionary,"columnMap"),MetadataLocaleProbe.field(dictionary,"supplementaryCharColumnMap"));
            shorts(out,(short[])MetadataLocaleProbe.field(dictionary,"rowIndex"));
            ints(out,(int[])MetadataLocaleProbe.field(dictionary,"rowIndexFlags"));
            shorts(out,(short[])MetadataLocaleProbe.field(dictionary,"rowIndexFlagsIndex"));
            byte[] shifts=(byte[])MetadataLocaleProbe.field(dictionary,"rowIndexShifts");out.writeInt(shifts.length);out.write(shifts);
            shorts(out,(short[])MetadataLocaleProbe.field(dictionary,"table"));
            boolean[] flags=(boolean[])MetadataLocaleProbe.field(thai,"categoryFlags");out.writeInt(flags.length);for(boolean flag:flags){out.writeBoolean(flag);}
        }
        List<String> cases=new ArrayList<>(Arrays.asList("","I","i","\u0130","\u0131","\u00df","\ufb03","\u03a3","A\u03a3","\u03a3A","A\u03a3A",
            "I\u0307","I\u0323\u0307","I\u0301\u0307","J\u0301","\u012e\u0323\u0301","\u00cc\u00cd\u0128","i\u0307","j\u0323\u0307",
            "AภาษาΣ","AประเทศไทยΣ","AΣกขค","AΣ\uffffA","A\u0000Σ","AΣ\u0000A","AΣ'A","AΣ.A","AΣ:A","AΣ\u0345",
            "\ud800AΣ\udc00","\ud801\udc00Σ","\ud801\udc28Σ","\ua7d0\ua7d1Σ","Σ\ud800\udc00A","ΣภาษาA","AภาษาΣA"));
        // Σ的上下文不能仅由相邻字符是否为字母决定；穷举短标点/组合符上下文。
        String[] contexts={"A","a","1","'",".",":","_","-"," ","\u0301","\u0345","\uffff","\ud800","\udc00","ภาษา","กขค"};
        for(String before:contexts){for(String after:contexts){cases.add(before+"Σ"+after);cases.add("A"+before+"Σ"+after+"A");}}
        char[] alphabet="AaIiJj\u0130\u0131Σσς\u00df\u0301\u0307\u0323\u0345'._- \u0000\uffff\ud800\udc00\ud801\udc28ภาษาไทยกขค".toCharArray();
        long seed=0x1ea7aL;
        for(int n=0;n<1000;n++){
            seed=(seed*1664525+1013904223)&0xffffffffL;int length=(int)(seed%24);char[] text=new char[length];
            for(int i=0;i<length;i++){seed=(seed*1664525+1013904223)&0xffffffffL;text[i]=alphabet[(int)(seed%alphabet.length)];}
            cases.add(new String(text));
        }
        String[] languages={"","en","tr","az","lt","th","el","xx"};int observations=0;
        try(PrintWriter out=new PrintWriter(Files.newBufferedWriter(output.resolve("jdk17-locale-observations.tsv"),StandardCharsets.UTF_8))){
            for(String language:languages){Locale locale=Locale.forLanguageTag(language);
                for(int i=0;i<cases.size();i++){
                    String text=cases.get(i);out.println("case/"+language+"/"+i+"\t"+units(text)+"\t"+units(text.toLowerCase(locale))+"\t"+units(text.toUpperCase(locale))+"\t"+boundary(text,locale));observations++;
                }
            }
        }
        try(PrintWriter out=new PrintWriter(Files.newBufferedWriter(output.resolve("locale-origin.tsv"),StandardCharsets.UTF_8))){
            out.println("JDK\t"+System.getProperty("java.version"));out.println("string_case_mapping_inputs\t"+records);
            out.println("computed_context_observations\t"+observations);out.println("cases_per_language\t"+cases.size());
            out.println("word_profile_locale_families\troot,th");out.println("default_language\t"+Locale.getDefault().getLanguage());
        }
    }
}
