import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.common.util.DateUtil;
import org.apache.seata.rm.datasource.undo.mysql.MySQLUndoLogManager;
import org.springframework.mock.env.MockEnvironment;

/** 原 Java 日期分界及真实 MySQL 3000 行删除方法的对照，不改原 SQL。 */
public final class UndoRetentionJavaOracle {
    /** 参数为 JDBC URL、保留天数和输出文件；返回真实批次数与分界。 */
    public static void main(String[] args) throws Exception {
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT,
                new MockEnvironment().withProperty("seata.config.type","file").withProperty("seata.config.file.name","file")
                        .withProperty("seata.registry.type","file"));
        int days=Integer.parseInt(args[1]);Date cutoff=DateUtil.getDateNowPlusDays(-(days<=0?7:days));
        List<Integer> batches=new ArrayList<>();
        try(Connection connection=DriverManager.getConnection(args[0]+"?useSSL=false&allowPublicKeyRetrieval=true&serverTimezone=UTC","root",
                Objects.requireNonNull(System.getenv("SEATA_MYSQL_PASSWORD")))) {
            MySQLUndoLogManager manager=new MySQLUndoLogManager();int rows;
            do {rows=manager.deleteUndoLogByLogCreated(cutoff,3000,connection);batches.add(rows);}while(rows==3000);
        }
        Files.write(Path.of(args[2]),new ObjectMapper().writeValueAsBytes(Map.of("days",days,"cutoff",new java.sql.Date(cutoff.getTime()).toString(),"batches",batches)));
    }
}
