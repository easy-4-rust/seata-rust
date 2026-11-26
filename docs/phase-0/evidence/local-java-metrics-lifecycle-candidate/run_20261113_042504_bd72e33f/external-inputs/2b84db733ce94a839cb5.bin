import java.io.PrintWriter;
import org.junit.platform.engine.discovery.DiscoverySelectors;
import org.junit.platform.launcher.core.LauncherDiscoveryRequestBuilder;
import org.junit.platform.launcher.core.LauncherFactory;
import org.junit.platform.launcher.listeners.SummaryGeneratingListener;

/** 使用既有固定 JUnit 运行目标测试；输出实际执行数量，拒绝跳过或空集合。 */
public final class ObservationJUnit {
    /** 参数为测试类全名；返回进程状态和完整失败详情。 */
    public static void main(String[] args) {
        SummaryGeneratingListener listener = new SummaryGeneratingListener();
        org.junit.platform.launcher.Launcher launcher = LauncherFactory.create();
        launcher.registerTestExecutionListeners(listener);
        LauncherDiscoveryRequestBuilder request = LauncherDiscoveryRequestBuilder.request();
        for (String name : args) request.selectors(DiscoverySelectors.selectClass(name));
        launcher.execute(request.build());
        org.junit.platform.launcher.listeners.TestExecutionSummary summary = listener.getSummary();
        summary.printTo(new PrintWriter(System.out, true));
        summary.printFailuresTo(new PrintWriter(System.out, true));
        System.out.println("OBSERVATION_JUNIT tests=" + summary.getTestsFoundCount()
                + " succeeded=" + summary.getTestsSucceededCount() + " failed=" + summary.getTestsFailedCount()
                + " skipped=" + summary.getTestsSkippedCount());
        System.exit(summary.getTestsFailedCount() == 0 && summary.getTestsFoundCount() > 0
                && summary.getTestsSkippedCount() == 0 ? 0 : 1);
    }
}
