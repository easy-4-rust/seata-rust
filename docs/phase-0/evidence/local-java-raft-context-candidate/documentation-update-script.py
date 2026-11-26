from pathlib import Path
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');BASE=ROOT/'docs/phase-0/evidence/local-java-raft-context-candidate'
def append(path,text):
 raw=path.read_bytes();extra=text.encode()
 if extra in raw:return
 with path.open('ab') as stream:stream.write(extra)
def put(path,text):
 data=text.encode()
 if path.exists():assert path.read_bytes()==data
 else:path.write_bytes(data)
put(BASE/'README.md','''# RAFT Prepare 上下文修复候选：真实恢复验证

依据[正式增量计划](../../java-raft-context-replication-candidate-plan.md)，主运行是 `run_20261113_064040_f5c35163`。独立0005补丁只供本候选；原Java仓库/JAR未改。此前[原实现真实复现](../local-go-java-raft-context-reference/README.md)证实：实际Prepare报告新增上下文已确认，原副本却保持注册时旧值；选主后真实回调拒绝业务确认，确认量/效果均0。该负向证据不是恢复通过。

候选在三节点统一加载相同补丁后完成自然16、两次停止真实领导并选主、六次快照与两次原盘加载、两次数据库/SDK重启。真实TCC确认10的原成功4丢失，AT真实清Undo后TC只留原TCC；同分支重放后确认仍10、效果仍1，余额900/Undo空，最终三副本会话/分支/锁均清理。

原11登记与原13报告确实不同；原13/14直接绑定12次副本TCC观察、三个实际回调阶段（TCC原帧5/1/1）、五份独立SQL准备token和两个SDK接受记录。全部回调帧逐条检查，未用副本彼此相同替代对原报告一致。5TC JVM/3Go SDK、35实际关键类来源、六protobuf元数据/两JFR重读、五原生只读查询30帧及停止身份/当前任期/进度分别校验。

新Java测试25项：21目标＋固定原回归4；同一测试class字节对原JAR类为11失败/14控制通过，对三候选类全部通过。705字节旧消息由原类真实序列化，新读取器应用后保留已有上下文。候选明确支持新值、空串和null清除；缺失/null/false标志保留旧status-only语义。

本运行102项Python全部通过，额外实际证据副本破坏检查10项通过，合计112项；102已含15项归一化guard测试，不重复计算。15项的RED14失败/1控制使用“guard未启用”的空适配器，仅证明校验器能拒绝错误样例，不能当成旧Java生产缺陷RED。生产行为缺陷依据原JAR的Java RED及真实原实现拒绝确认。原实现另有10项复现证据检查，独立计数。Go254/Rust117及旧Java RAFT52等父范围继承，未声称本轮重跑。

1039运行输入在启动前冻结；341classpath/2821依赖文件、23生产覆盖类＋1只读控制类、3只读辅助类，均与已测试字节及原依赖前后一致。Java单测另有278classpath/1950依赖。额外校验10输入独立冻结在post-run-validation；最终认证须同时通过原运行与追加实际破坏门禁。开发及证据均在D/E，自有容器/进程停止、11端口释放。

仅认证3000ms选举profile的统一补丁组，新reader兼容原status-only消息。不认证混合版本滚动升级，不重建历史已丢失上下文。原参考问题仍OPEN；1000ms分票、metadata/logging、指标/Exporter、全分区/并发/损坏/崩溃、通用Fence、生产认证、正式Rust、完整WP00及Phase1–7仍分别验收。分阶段冻结提案仍待确认。
''')
append(ROOT/'docs/delivery-progress.md','''

## RAFT Prepare 上下文真实缺陷与0005修复候选（2026-11-13）

依[增量计划](phase-0/java-raft-context-replication-candidate-plan.md)，先完成[原实现活体复现](phase-0/evidence/local-go-java-raft-context-reference/README.md)：真实Prepare生成并报告新token，原主节点持有新值而两个副本仍旧值；实际选主后原回调缺值，业务拒绝确认，确认/效果0。该运行只证明缺陷，原参考问题JAVA-RAFT-BRANCH-CONTEXT-001保持OPEN。

[0005候选主运行064040](phase-0/evidence/local-java-raft-context-candidate/README.md)以同一业务二进制和统一三节点补丁完成两次实际选主、六snapshot/两原盘加载、两次DB/SDK重启、真实成功回执丢失及持久重放。原13/14更新context直接绑定12副本观察、所有TCC回调（阶段帧数5/1/1）、五SQL token和两实际接受记录。最终确认10/效果1、余额900/Undo空且三副本清理；5TC/3Go SDK、35关键类来源、两JFR与五原生查询30帧分别核验。

新Java25：同测试class对原类11失败/14控制，候选25通过；真实原类生成的705B旧status-only消息控制通过。Python102运行＋10实际证据破坏＝112，15归一化guard已含在102中；其RED14/1为空guard适配器控制，不是旧生产RED。1039冻结输入、341classpath/2821依赖、同23生产类＋1只读控制与3辅助类闭合；资源停止、11端口释放。Go254/Rust117及旧Java父测试明确继承。

只认证统一补丁和新reader读旧消息，不认证混合滚升或历史丢失数据修复；1000ms分票和其他原参考/指标/全故障矩阵/认证仍开放。正式Rust、完整WP00与Phase1–7未完成，冻结提案仍待确认；继续其余既定门禁。
''')
for name in ['README.md','validation-ledger.md','java-tc-retry-timeout-recovery.md']:
 append(ROOT/'docs/phase-0'/name,'''

## RAFT Prepare 上下文更新修复候选（2026-11-13）

[计划](java-raft-context-replication-candidate-plan.md)对应[原实现真实缺陷](evidence/local-go-java-raft-context-reference/README.md)与[0005候选064040](evidence/local-java-raft-context-candidate/README.md)。原11/13确实不同，原副本保留旧上下文，选主后实际TCC拒绝确认、业务效果0；负向运行不计恢复成功。候选统一补丁组三节点在两次领导更替、六snapshot/两原盘加载、两次DB/SDK重启及真实成功回复丢失后，同原分支持久重放确认仍10/效果1，最终三副本清理，余额900/Undo空。

直接绑定原13/14→12副本观察→全部真实TCC回调（阶段5/1/1）→五SQL准备token及两个接受记录；5TC/3Go SDK、35实际类来源、两JFR重读和五原生查询30帧完整核验。Java同字节25测试原类RED11失败/14控制、候选25通过，真实原类705B旧消息兼容通过。Python102＋追加实际证据破坏10＝112；归一化15已包含，空guard适配器RED仅作校验器负控，不冒充原生产RED。1039运行输入＋10追加验证输入、341classpath/2821依赖、23生产覆盖类/1只读控制/3辅助类前后闭合；资源停止、11端口释放。Go254/Rust117及旧Java父范围继承未重跑。

JAVA-RAFT-BRANCH-CONTEXT-001原参考仍OPEN，独立0005取得选定范围证据；只认证统一补丁与新reader读旧消息，不支持混合版本滚升声明或修复历史已丢上下文。1000ms分票、metadata/logging、指标/Exporter、全故障矩阵/通用Fence/认证、正式Rust、完整WP00和Phase1–7继续开放；分阶段冻结提案仍待确认。
''')
append(ROOT/'docs/phase-0/raft-commit-runtime-findings.md','''

## 后续真实上下文更新复现与候选验证（2026-11-13）

上文“静态风险/未活体验收”描述的是055235不变context样例的范围。随后按[独立计划](java-raft-context-replication-candidate-plan.md)完成[061711原实现复现](evidence/local-go-java-raft-context-reference/README.md)：Prepare在真实SQL提交后生成新token并通过原13/14报告，原领导持新值、副本持原11旧值；真实选主后回调缺值，真实业务拒绝确认，SQL确认量/效果0。这将JAVA-RAFT-BRANCH-CONTEXT-001升级为实际观察缺陷，原参考保持OPEN。

[064040候选证据](evidence/local-java-raft-context-candidate/README.md)以显式nullable更新标志携带状态与context，新follower仅TRUE应用值（含空/null），旧标志缺失/null/false保持原context。同字节Java25原类11失败/14控制→候选全通过，真实旧序列化blob兼容。统一补丁组三节点两次选主与快照/重启/丢成功回执后，更新context仍与原13及原SQL完全绑定，真实确认/持久重放效果1；112 Python和35类来源等证据详见主目录。

由于固定Jackson拒绝未知字段，候选仅认证全节点同补丁；新reader读旧消息不等于旧reader能读新消息，不声称混合滚升，也不重建历史丢失数据。其他原参考与1000ms分票问题不随本候选通过而关闭。
''')
append(ROOT/'compatibility/seata-java/README.md','''

## 0005：显式复制分支上下文更新

[补丁](cba73e180fd8896a99df6415b859d275122feaa7/0005-replicate-explicit-branch-context-updates.patch)及独立manifest覆盖BranchTransactionDTO、RaftSessionManager、UpdateBranchSessionExecute三个真实对象。详见[计划](../../docs/phase-0/java-raft-context-replication-candidate-plan.md)和[真实证据](../../docs/phase-0/evidence/local-java-raft-context-candidate/README.md)。原JAR/固定仓库未改；本运行使用前导/保留/reload20生产类＋本补丁3类，共23生产类，未加入0004指标类，不能据此认证RAFT指标。

显式TRUE复制当前context（支持值/空/null）；历史missing/null/false标志保持旧语义。同测试class Java25原类11失败/14控制、候选全过，原类真实生成705B旧消息兼容；真实更改Prepare context后三节点完整恢复通过。仅统一补丁组，禁止据此宣称混合版本滚动升级或历史丢失上下文恢复；原参考问题继续OPEN。
''')
print('Appended current evidence and scope; historical source snapshots unchanged. Do not rerun prior source-current archivers after this documentation epoch.')
