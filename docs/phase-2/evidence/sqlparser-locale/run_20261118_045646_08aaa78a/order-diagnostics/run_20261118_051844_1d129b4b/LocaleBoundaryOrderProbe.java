import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.text.BreakIterator;
import java.util.Locale;

/** 定位“按升序预计算isBoundary”与原Σ查询顺序的真实反例；仅诊断，不计原测试覆盖。 */
public final class LocaleBoundaryOrderProbe {
    private static final String[] PARTS={"A","Σ","\ud800","\udc00","\ud800\udc00","\uffff","_","'","ก","\u0345","\u0301","\u0000","\ud801\udc28","\u1d2c"};
    private static Method isCased;
    private static long checked;
    private static String units(String text){StringBuilder out=new StringBuilder();for(char ch:text.toCharArray()){out.append(String.format("%04x",(int)ch));}return out.toString();}
    private static String cached(String text,Locale locale)throws Exception{
        BreakIterator word=BreakIterator.getWordInstance(locale);word.setText(text);boolean[] flags=new boolean[text.length()+1];
        for(int i=0;i<flags.length;i++){flags[i]=word.isBoundary(i);}
        StringBuilder out=new StringBuilder();
        for(int index=0;index<text.length();){int cp=text.codePointAt(index),next=index+Character.charCount(cp);boolean terminal=false;
            if(cp==0x3a3){for(int before=index;!flags[before];){int prev=text.codePointBefore(before);before-=Character.charCount(prev);
                if((Boolean)isCased.invoke(null,prev)){terminal=true;
                    for(int after=next;after<text.length()&&!flags[after];){int following=text.codePointAt(after);if((Boolean)isCased.invoke(null,following)){terminal=false;break;}after+=Character.charCount(following);}break;
                }
            }}
            out.append(terminal?"ς":text.substring(index,next).toLowerCase(locale));index=next;
        }return out.toString();
    }
    private static String find(String text,int remaining,Locale locale)throws Exception{
        if(text.indexOf('Σ')>=0){checked++;String actual=text.toLowerCase(locale),changed=cached(text,locale);
            if(!actual.equals(changed)){return locale.toLanguageTag()+"\t"+units(text)+"\t"+units(actual)+"\t"+units(changed);}}
        if(remaining==0){return null;}
        for(String part:PARTS){String found=find(text+part,remaining-1,locale);if(found!=null){return found;}}
        return null;
    }
    public static void main(String[] args)throws Exception{
        isCased=Class.forName("java.lang.ConditionalSpecialCasing").getDeclaredMethod("isCased",int.class);isCased.setAccessible(true);
        String found=find("",5,Locale.ROOT);if(found==null){found=find("",5,Locale.forLanguageTag("th"));}
        Files.writeString(Path.of(args[0]),"checked\t"+checked+"\n"+(found==null?"NO_COUNTEREXAMPLE_IN_DECLARED_SEARCH":found)+"\n",StandardCharsets.UTF_8);
        System.out.println("checked="+checked+" found="+(found!=null));
    }
}
