import com.fasterxml.jackson.databind.ObjectMapper;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.apache.seata.core.lock.Locker;
import org.apache.seata.core.lock.RowLock;
import org.apache.seata.server.lock.AbstractLockManager;
import org.apache.seata.server.session.BranchSession;
import org.apache.seata.server.session.GlobalSession;

/** 真实原TC方法与录制后端；不发送网络请求，不代表真实锁存储验收。 */
public final class LockCoverageOracle extends AbstractLockManager {
    private int backendCalls;
    private List<Map<String,String>> delegated=List.of();
    @Override protected Locker getLocker(BranchSession session) {
        return (Locker)Proxy.newProxyInstance(Locker.class.getClassLoader(),new Class<?>[]{Locker.class},(proxy,method,args)->{
            if("acquireLock".equals(method.getName())) {
                backendCalls++;
                List<?> values=(List<?>)args[0];List<RowLock> rows=new ArrayList<>();
                for(Object value:values)rows.add((RowLock)value);
                delegated=rows(rows);return true;
            }
            throw new UnsupportedOperationException("Unused recording backend method");
        });
    }
    @Override public boolean releaseGlobalSessionLock(GlobalSession session) {
        throw new UnsupportedOperationException("Unused recording release");
    }
    private static List<Map<String,String>> rows(List<RowLock> values) {
        List<Map<String,String>> rows=new ArrayList<>();
        for(RowLock value:values)rows.add(Map.of("table",value.getTableName(),"pk",value.getPk()));
        return rows;
    }
    public static void main(String[] args) throws Exception {
        ObjectMapper mapper=new ObjectMapper();List<?> cases=mapper.readValue(Files.readAllBytes(Path.of(args[0])),List.class);
        List<Object> results=new ArrayList<>();
        for(Object raw:cases) {
            Map<?,?> input=(Map<?,?>)raw;LockCoverageOracle manager=new LockCoverageOracle();BranchSession branch=new BranchSession();
            branch.setXid("fixture:123");branch.setTransactionId(123L);branch.setBranchId(42L);branch.setResourceId("owned-fixture");branch.setLockKey((String)input.get("key"));
            List<Map<String,String>> parsed=rows(manager.collectRowLocks(branch));boolean acquired=manager.acquireLock(branch);
            if(!Objects.equals(input.get("expected_rows"),parsed)||!Objects.equals(input.get("expected_backend_calls"),manager.backendCalls)||!acquired)
                throw new AssertionError("Original source observation differs: "+input.get("case"));
            if(manager.backendCalls>0&&!Objects.equals(parsed,manager.delegated))throw new AssertionError("Backend received different rows");
            results.add(Map.of("case",input.get("case"),"key",input.get("key"),"parsed_rows",parsed,
                "acquire_returned",acquired,"recording_backend_calls",manager.backendCalls,"delegated_rows",manager.delegated));
        }
        Files.write(Path.of(args[1]),mapper.writerWithDefaultPrettyPrinter().writeValueAsBytes(results));
        System.out.println("original_source_observations_verified="+results.size()+" recording_backend=true safety_gate_passed=false");
        System.exit(0);
    }
}
