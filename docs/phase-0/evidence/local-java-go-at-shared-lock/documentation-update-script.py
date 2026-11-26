from pathlib import Path
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-java-go-at-shared-lock'
def append(path,text):
 raw=path.read_bytes();data=text.encode()
 if data in raw:return
 with path.open('ab') as stream:stream.write(data)
def put(path,text):
 data=text.encode()
 if path.exists():assert path.read_bytes()==data
 else:path.write_bytes(data)
put(BASE/'README.md','''# Java / Go AT 同一行共享锁：真实缺陷调查

按[现有增量计划](../../java-go-at-shared-lock-reference-plan.md)，正式负向主运行是 `run_20261113_070224_d7adfec7`。它证明GO-AT-RESOURCE-ID-001的实际风险，不是安全互操作通过，也不是修复或正式Rust组件。

固定Java DataSourceProxy真实AT先把同一accounts主键行1000改为900，本地提交并持有原全局Begin/分支/Undo/TC锁；固定Go SDK随后原Begin、真实seata-at-mysql把同一行900改为800。独立SQL同时读取两个原XID/BranchId及各自非空原Undo，两个全局事务均仍为Begin。没有修改线上resource/lock字段或合成RM成功。

| 观察阶段 | Java资源＋Java锁键 | Go资源＋Java锁键 | Java资源＋Go锁键 | Go资源＋Go锁键 |
|---|---|---|---|---|
| 仅Java阶段一提交，Go已Begin但未执行SQL | 不可锁 | 可锁 | 可锁 | 可锁 |
| 两个阶段一均已提交，全局均Begin | 不可锁 | 可锁 | 可锁 | 不可锁 |
| 两个原TM提交、AT清Undo及TC终态后 | 可锁 | 可锁 | 可锁 | 可锁 |

每格均为原生只读101/15/21、独立锁查询身份及原TC响应，不借同XID重入。固定Java锁键仅换resource，原锁即不可见，独立证明资源命名空间差异；固定Java资源换为Go实际锁键也可锁，说明键文本差异另需兼容。此实验不分开证明大写与尾分号各自的作用，不把全部URL/大小写/别名问题一并宣布解决。

实际Java资源为jdbc:mysql URL去查询参数；实际Go资源含测试DSN凭据。Java锁键`accounts:1`，Go为`ACCOUNTS:1;`。证据仅使用自有容器固定测试凭据，不接触外部数据库。最终通过各自原TM正常提交清理，余额800/Undo0/原事务缺席且锁释放；不交叉回滚，不认证两种Undo格式互换、dirty image补偿或旧标识迁移。

原Java客户端默认发送V2，Go发送V1；透明代理保留原版本和全部原始字节，分别严格解析102/104格式，没有把Java V2帧改写后冒充V1认证。原TC仍为同一固定JAR，仅加载已认证10class前导候选；实际FILE锁管理/ATCore/SessionHolder均从原JAR加载。本场景不扩大Rust的V2产品边界。

1个TC JVM、1个真实Java客户端JVM、1个Go SDK；118运行输入、342classpath/2802依赖文件与原JAR/SDK候选/actor二进制前后核对。16实际类来源、6个关键RM/TM源逐LF等于原pin和D构建副本、对应6class等于原参考target字节；Java actor编译产物结束复核。Java12帧/Go48帧及12只读查询72帧独立重放。

本轮32 Python=12归一化＋20实际证据破坏检查。12的RED11失败/1控制只是未启用guard适配器控制；20的RED18失败/2控制来自原冻结runtime校验器，补强后全部通过，证明新增Begin/Commit/报告/类来源/SQL门禁有效。两种RED均不是原Java生产单元测试失败。Go254/Rust117单元父范围继承，未声称本轮重跑。

最终认证必须同时通过原运行与追加门禁；32个追加输入独立冻结，不篡改118输入的原epoch。原实现缺陷仍OPEN，安全共享锁/稳定无凭据标识/锁键兼容/资源迁移/正式Rust与完整WP00未完成。自有进程与容器停止，28091/28092/28093/28094四端口释放。早期空classpath、缺Driver/Lombok及错误V1-only代理失败保留为工具诊断，均不算缺陷复现通过。
''')
append(ROOT/'docs/phase-0/go-at-resource-identity-divergence.md','''

## 真实跨语言共享行验证（2026-11-13）

此前“风险推断、尚未争锁”是初始审计状态；[070224真实证据](evidence/local-java-go-at-shared-lock/README.md)已按[计划](java-go-at-shared-lock-reference-plan.md)复现：原Java DataSourceProxy与原Go seata-at-mysql在同一MySQL行先后完成本地阶段一，同时保留各自Begin/分支/非空Undo；余额1000→900→800。固定Java实际锁键仅切换Go resource后原锁不可见，独立证明不同资源标识隔离全局锁命名空间。固定Java资源换实际Go锁键也可锁，键文本不一致仍是独立兼容项；未分开归因大小写与尾分号各自的影响。

原Java客户端实际默认V2、Go为V1，透明代理原样转发并分别严格解析；不冒充Java V1 RM验证或Rust V2能力。12原生查询72帧、16类来源、6关键源/类原pin验证、118运行/32追加输入、342classpath/2802依赖文件，以及32 Python门禁均通过独立复审。原TM正常提交只负责清理：最终余额800/Undo空且锁释放，不认证安全互斥、Undo互换或资源迁移。

GO-AT-RESOURCE-ID-001仍OPEN，现从格式风险推断升级为所选真实跨语言组合的实际锁绕过。下一步须实现显式稳定无凭据资源与Java等价锁键方案，并重跑相同物理行的冲突拒绝、提交/补偿、重连与持久恢复；不能仅修资源字符串而忽略锁键或旧Undo/缓存。
''')
append(ROOT/'docs/delivery-progress.md','''

## Java / Go AT 真实共享行锁缺口（2026-11-13）

按[共享锁调查计划](phase-0/java-go-at-shared-lock-reference-plan.md)完成[070224负向证据](phase-0/evidence/local-java-go-at-shared-lock/README.md)：真实Java DataSourceProxy先阶段一持全局锁，原Go SDK仍在同一MySQL行完成独立阶段一，余额1000→900→800且两个原Undo同时可见。保持Java锁键、仅切换资源标识，原锁查询从不可锁变为可锁，独立证明命名空间隔离；实际锁键文本差异也需处理。本项是实际缺陷复现，不是安全互操作通过，GO-AT-RESOURCE-ID-001继续OPEN。

1TC/1Java客户端/1Go SDK；实际Java默认V2与Go V1帧原样保留、分别解析，不扩张Rust V2范围。118运行输入＋32追加输入、342classpath/2802依赖、16类来源/6关键原源类验证、60原SDK帧及12原生查询72帧闭合；Python32本轮执行。原TM提交清理后余额800/Undo空/锁释放，自有资源停止，四端口释放。Go254/Rust117单元继承未重跑。下一步为显式共享资源与锁键兼容方案及拒绝冲突/恢复验证；正式Rust、完整WP00和Phase1–7继续未完成，冻结提案仍待确认。
''')
for name in ['README.md','validation-ledger.md','wp00-freeze-readiness-audit.md']:
 append(ROOT/'docs/phase-0'/name,'''

## Java / Go AT 共享锁实际缺陷（2026-11-13）

[计划](java-go-at-shared-lock-reference-plan.md)对应[070224负向主证据](evidence/local-java-go-at-shared-lock/README.md)。真实Java与Go AT在同一MySQL行同时保留已提交阶段一，余额1000→900→800、两个原Undo并存；固定Java锁键仅换resource即可看不到原锁，资源命名空间隔离由实际查询证明，锁键文本差异仍单独开放。GO-AT-RESOURCE-ID-001由源码推断升级为所选组合实际锁绕过，未认证安全互斥或迁移。

32 Python=归一化12＋实际证据破坏20；后者旧冻结校验器RED18失败/2控制、新门禁20通过。118运行/32追加输入、342classpath/2802依赖、16类来源、6原关键源/类、12原生查询72帧及真实TM/报告/原Undo分别闭合。Java客户端默认V2、Go V1原样记录，未扩张V1认证边界。原TM提交清理后余额800/Undo空/锁释放，3自有进程及容器退出、四端口释放。Go254/Rust117单元继承无新执行；正式Rust、WP00和全计划其余门禁仍未完成。
''')
print('Recorded actual shared-lock defect without changing frozen runtime sources; do not rerun historical current-source archivers.')
