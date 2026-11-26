import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.core.model.BranchStatus;
import org.apache.seata.server.cluster.raft.sync.RaftSyncMessageSerializer;
import org.apache.seata.server.cluster.raft.sync.msg.RaftBranchSessionSyncMsg;
import org.apache.seata.server.cluster.raft.sync.msg.RaftSyncMessage;
import org.apache.seata.server.cluster.raft.sync.msg.RaftSyncMsgType;
import org.apache.seata.server.cluster.raft.sync.msg.dto.BranchTransactionDTO;
import org.springframework.mock.env.MockEnvironment;

/** 必须使用原DTO类生成实际旧RAFT消息，不通过删除新JSON字段模拟历史消息。 */
public final class LegacyRaftContextBlob {
    /** 参数为D/E上不存在的输出文件；写入原序列化器产生的完整RAFT消息。 */
    public static void main(String[] args) throws Exception {
        Path output = Path.of(args[0]).toAbsolutePath().normalize();
        if (!output.getParent().toRealPath().toString().matches("^[DE]:[\\\\/].*")) {
            throw new IllegalArgumentException("Owned D/E output required");
        }
        try {
            BranchTransactionDTO.class.getMethod("getApplicationDataUpdated");
            throw new IllegalStateException("Legacy producer must load the unchanged original DTO");
        } catch (NoSuchMethodException expectedOriginalClass) {
            // 确认真实原类没有候选字段后才生成旧消息。
        }
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT,
                new MockEnvironment().withProperty("seata.config.type", "file")
                        .withProperty("seata.config.file.name", "file"));
        BranchTransactionDTO dto = new BranchTransactionDTO("127.0.0.1:28291:710001", 710002L);
        dto.setStatus(BranchStatus.PhaseOne_Done.getCode());
        RaftSyncMessage message = new RaftSyncMessage();
        message.setBody(new RaftBranchSessionSyncMsg(RaftSyncMsgType.UPDATE_BRANCH_SESSION_STATUS, dto));
        Files.write(output, RaftSyncMessageSerializer.encode(message), StandardOpenOption.CREATE_NEW);
    }
}
