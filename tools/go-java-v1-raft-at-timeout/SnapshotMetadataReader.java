import com.alipay.sofa.jraft.entity.LocalStorageOutter.LocalSnapshotPbMeta;
import com.alipay.sofa.jraft.storage.io.ProtoBufFile;
import java.nio.file.Path;

/** 只读解析已归档的真实JRaft protobuf元数据，不加载会话或启动节点。 */
public final class SnapshotMetadataReader {
    /** 参数为D/E上的元数据文件；返回JSON数组到标准输出，保留实际index与term。 */
    public static void main(String[] args) throws Exception {
        StringBuilder out = new StringBuilder("[");
        for (int index = 0; index < args.length; index++) {
            Path path = Path.of(args[index]).toRealPath();
            if (!path.toString().matches("^[DE]:[\\\\/].*")) {
                throw new IllegalArgumentException("Owned D/E snapshot metadata required");
            }
            LocalSnapshotPbMeta value = new ProtoBufFile(path.toString()).load();
            if (index > 0) {
                out.append(',');
            }
            out.append("{\"index\":").append(value.getMeta().getLastIncludedIndex())
                    .append(",\"term\":").append(value.getMeta().getLastIncludedTerm()).append('}');
        }
        System.out.println("SNAPSHOT_METADATA_JSON=" + out.append(']'));
    }
}
