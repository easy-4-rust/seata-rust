import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.*; import java.util.*; import java.lang.reflect.Proxy;
import org.apache.seata.server.lock.AbstractLockManager;
import org.apache.seata.server.session.BranchSession;
import org.apache.seata.server.session.GlobalSession;
import org.apache.seata.core.lock.Locker;
import org.apache.seata.core.lock.RowLock;
public final class LockCoverageDiagnostic extends AbstractLockManager {
 private int backendCalls;
 @Override protected Locker getLocker(BranchSession session) { return (Locker)Proxy.newProxyInstance(Locker.class.getClassLoader(),new Class<?>[]{Locker.class},(proxy,method,args)->{ if(method.getName().equals("acquireLock")){backendCalls++;return true;} throw new UnsupportedOperationException("Unused recording backend method"); }); }
 @Override public boolean releaseGlobalSessionLock(GlobalSession session) { throw new UnsupportedOperationException("Unused recording release"); }
 public static void main(String[] args) throws Exception {
  List<Object> out=new ArrayList<>();
  for(String key:List.of("t:1","t:","t: ","t:x;broken;later:2","t:[0, -1]","t:x;later:2")) {
   LockCoverageDiagnostic instance=new LockCoverageDiagnostic(); BranchSession branch=new BranchSession(); branch.setXid("fixture:123");branch.setTransactionId(123L);branch.setBranchId(42L);branch.setResourceId("owned-fixture");branch.setLockKey(key);
   List<RowLock> locks=instance.collectRowLocks(branch);boolean result=instance.acquireLock(branch);List<Object> rows=new ArrayList<>();for(RowLock row:locks)rows.add(Map.of("table",row.getTableName(),"pk",row.getPk()));
   out.add(Map.of("key",key,"parsed_rows",rows,"acquire_returned",result,"recording_backend_calls",instance.backendCalls));
  }
  Files.write(Path.of(args[0]),new ObjectMapper().writerWithDefaultPrettyPrinter().writeValueAsBytes(out));System.out.println("original_source_parser_scenarios="+out.size());System.exit(0);
 }
}