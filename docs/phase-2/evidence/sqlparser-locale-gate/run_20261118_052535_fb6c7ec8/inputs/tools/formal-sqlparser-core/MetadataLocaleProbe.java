import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.text.BreakIterator;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/** 只读探查固定JDK的Locale分词与大小写契约；仅辅助类使用反射，不改原Seata。 */
public final class MetadataLocaleProbe {
    static Object field(Object object,String name) throws Exception {
        for(Class<?> type=object.getClass();Objects.nonNull(type);type=type.getSuperclass()){
            try{Field f=type.getDeclaredField(name);f.setAccessible(true);return f.get(object);}
            catch(NoSuchFieldException ignored){ /* 继续父类中的原私有字段。 */ }
        }
        throw new NoSuchFieldException(name);
    }
    static byte[] profile(BreakIterator iterator) throws Exception {
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream data=new DataOutputStream(bytes);
        data.writeInt((Integer)field(iterator,"numCategories"));
        for(String name:List.of("stateTable","backwardsStateTable")){
            short[] values=(short[])field(iterator,name);data.writeInt(values.length);for(short value:values){data.writeShort(value);}
        }
        for(String name:List.of("endStates","lookaheadStates")){
            boolean[] values=(boolean[])field(iterator,name);data.writeInt(values.length);for(boolean value:values){data.writeBoolean(value);}
        }
        Object compact=field(iterator,"charCategoryTable");
        short[] indices=(short[])field(compact,"indices");byte[] values=(byte[])field(compact,"values");
        data.writeInt(indices.length);for(short value:indices){data.writeShort(value);}
        data.writeInt(values.length);data.write(values);
        int[] supplementary=(int[])field(field(iterator,"supplementaryCharCategoryTable"),"dataTable");
        data.writeInt(supplementary.length);for(int value:supplementary){data.writeInt(value);}
        return bytes.toByteArray();
    }
    private static String units(String text){StringBuilder out=new StringBuilder();for(char ch:text.toCharArray()){out.append(String.format("%04x",(int)ch));}return out.toString();}
    public static void main(String[] args)throws Exception{
        Path out=Path.of(args[0]);Files.createDirectories(out);
        Map<String,StringBuilder> families=new LinkedHashMap<>();
        for(Locale locale:BreakIterator.getAvailableLocales()){
            BreakIterator word=BreakIterator.getWordInstance(locale);byte[] data=profile(word);
            String hash=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));
            String key=word.getClass().getName()+":"+hash;
            if(!families.containsKey(key)){families.put(key,new StringBuilder());Files.write(out.resolve(hash+".bin"),data);}
            families.get(key).append(locale.toLanguageTag()).append(',');
        }
        StringBuilder text=new StringBuilder();for(Map.Entry<String,StringBuilder> row:families.entrySet()){text.append(row.getKey()).append('\t').append(row.getValue()).append('\n');}
        text.append("default\t").append(Locale.getDefault().toLanguageTag()).append('\n');
        for(String language:List.of("","en","tr","az","lt","th","ja","el")){
            Locale locale=Locale.forLanguageTag(language);
            for(String input:List.of("AภาษาΣ","AประเทศไทยΣ","AΣกขค","I\u0307","I\u0323\u0307","I\u0301\u0307","AΣ\uffffA")){
                text.append(language).append('\t').append(units(input)).append('\t').append(units(input.toLowerCase(locale))).append('\t');
                BreakIterator word=BreakIterator.getWordInstance(locale);word.setText(input);
                for(int i=0;i<=input.length();i++){text.append(word.isBoundary(i)?'1':'0');}text.append('\n');
            }
        }
        Files.writeString(out.resolve("probe.tsv"),text,StandardCharsets.UTF_8);
    }
}
