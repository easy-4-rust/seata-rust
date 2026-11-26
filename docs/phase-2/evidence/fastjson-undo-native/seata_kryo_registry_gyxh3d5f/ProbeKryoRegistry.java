import com.esotericsoftware.kryo.Kryo;
import org.apache.seata.rm.datasource.undo.parser.*;
class ProbeKryoRegistry {
    public static void main(String[] args) throws Exception {
        var parser = new KryoUndoLogParser();
        parser.init();
        var wrapper = KryoSerializerFactory.getInstance().get();
        var field = KryoSerializer.class.getDeclaredField("kryo");
        field.setAccessible(true);
        var kryo = (Kryo)field.get(wrapper);
        for (int id = 0; id < kryo.getNextRegistrationId(); id++) {
            var item = kryo.getRegistration(id);
            if (item != null) {
                System.out.printf("%d\t%s\t%s%n", id,
                        item.getType().getName(), item.getSerializer().getClass().getName());
            }
        }
        KryoSerializerFactory.getInstance().returnKryo(wrapper);
    }
}
