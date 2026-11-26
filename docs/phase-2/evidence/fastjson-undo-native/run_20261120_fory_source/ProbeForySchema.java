import org.apache.fory.Fory;
import org.apache.fory.config.*;
import org.apache.fory.meta.ClassDef;
import org.apache.seata.rm.datasource.undo.*;
import org.apache.seata.rm.datasource.sql.struct.*;
import java.util.*;

class ProbeForySchema {
    public static void main(String[] args) {
        var codec = Fory.builder().withLanguage(Language.JAVA)
            .requireClassRegistration(false).withRefTracking(true)
            .withClassLoader(ProbeForySchema.class.getClassLoader())
            .withCompatibleMode(CompatibleMode.COMPATIBLE).build();
        for (Class<?> type : new Class<?>[] {BranchUndoLog.class, SQLUndoLog.class,
                TableRecords.class, Row.class, Field.class, TableRecords.EmptyTableRecords.class}) {
            ClassDef def = ClassDef.buildClassDef(codec, type);
            System.out.printf("class\t%s\t%s%n", type.getName(), HexFormat.of().formatHex(def.getEncoded()));
            for (ClassDef.FieldInfo field : def.getFieldsInfo()) {
                System.out.printf("field\t%s\t%s%n", type.getSimpleName(), field);
            }
        }
        for (Class<?> type : new Class<?>[] {ArrayList.class,Integer.class,Long.class,String.class,Boolean.class,Double.class}) {
            System.out.printf("registered\t%s\t%d%n", type.getName(), codec.getClassResolver().getRegisteredClassId(type));
        }
    }
}
