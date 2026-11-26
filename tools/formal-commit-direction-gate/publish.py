"""Publish only an independently reviewed immutable selected-component qualification."""
import hashlib,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def read(p):return json.loads(p.read_bytes())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def need(v,why):
    if not v:raise ValueError(why)
def write(p,value):
    data=(json.dumps(value,ensure_ascii=False,indent=2)+'\n').encode('utf-8') if not isinstance(value,str) else value.encode('utf-8')
    p.parent.mkdir(parents=True,exist_ok=True)
    if p.exists():need(p.read_bytes()==data,'Refuse replacing publication '+str(p))
    else:
        with p.open('xb') as f:f.write(data)
def run(packet,review_path,audit_path):
    packet=packet.resolve();review_path=review_path.resolve();audit_path=audit_path.resolve();base=ROOT/'docs/phase-2/evidence/mysql-commit-direction-gate'
    need(packet.parent==base.resolve(),'Qualification path differs')
    result=read(packet/'result.json');review=read(review_path);audit=read(audit_path);digest=sha(packet/'catalog.json')
    need(result['status']=='SELECTED_MANAGED_COMMIT_CLEANUP_VERIFIED_FULL_COMMIT_GATES_OPEN' and result['managed_commit_cleanup_verified'] and result['actual_target_behavior_runtime_RED_verified'],'Selected qualification did not pass')
    need(review['catalog_sha256']==digest and review['unresolved_P1']==review['unresolved_P2']==0 and review['verdict']=='PASS_SELECTED_SCOPE_ONLY','Final independent review does not approve this exact packet')
    members=read(packet/'catalog.json')['members']
    for row in members:need(sha(packet/row['path'])==row['sha256'],'Sealed member differs')
    for flag in ['public_first_phase_business_COMMIT_available','first_phase_SQL_Undo_commit_atomicity_verified','all_old_first_phase_attempts_fenced','commit_direction_verified','TC_safety_gate_passed','WP05_complete','WP06_complete','full_V1_complete','external_TC_risk_accepted']:need(result[flag] is False,'Publication overclaims full scope')
    need(all(v['verified_absent'] and Path(v['D_data_retained']).is_dir() for v in audit['containers']) and all(v['verified_owned_processes_absent'] for v in audit['java']),'All-attempt runtime audit incomplete')
    def rel(p):return p.relative_to(ROOT).as_posix()
    state=result|dict(specification='docs/phase-2/mysql-commit-direction-contract.md',parent_specification='docs/phase-2/wp05-mysql-at-plan.md',baseline_HEAD='130ad634279cef10cd2998fc3bfae82996224d66',Java_oracle_pin='cba73e180fd8896a99df6415b859d275122feaa7',evidence=rel(packet),catalog_sha256=digest,members=len(members),readonly_review=rel(review_path),readonly_review_sha256=sha(review_path),all_attempt_runtime_audit=rel(audit_path),all_attempt_runtime_audit_sha256=sha(audit_path),all_attempt_containers_confirmed_absent=audit['owned_containers_verified_absent'],all_attempt_Java_runs_confirmed_stopped=audit['owned_java_runs_verified_stopped'],original_seven_column_Undo_schema_changed=False,new_schema_migration_added=True,old_writers_must_stop_before_explicit_migration=True,full_AsyncWorker_migrated=False,full_source_Java_migration_complete=False,original_Java_source_test_files=189,original_Java_static_test_method_declarations=1135,production_auth_TLS_qualified=False,permission_trace_SQL_TEXT_preserved_as_null=True,additional_status_diagnostic_runtime_audit='docs/phase-2/evidence/mysql-commit-direction/status-diagnostic-audit-20261117.json',additional_status_diagnostic_runtime_audit_sha256=sha(ROOT/'docs/phase-2/evidence/mysql-commit-direction/status-diagnostic-audit-20261117.json'),next='Complete gated first-phase business COMMIT design and original TC exclusive-lock coverage; full source migration remains open')
    state_path=ROOT/'docs/phase-2/wp05-mysql-commit-direction.json';write(state_path,state)
    rows='\n'.join('| '+rust+' | '+str(v['passed'])+' | '+str(v['harness_artifacts'])+' | '+str(v['doc_tests'])+' | 53 |' for rust,v in result['ordinary'].items())
    text=f"""# WP05：正式提交回调的持久清理验收

本组件已验证：正式 TC 提交回调持久授权后，原 root／子片清理与提交终态在同一控制事务完成；实际 COMMIT 确认后才发布完成。业务值不变。公开一阶段业务 COMMIT、完整异步批调度、完整 WP05／WP06／V1 继续 OPEN。

沿用 [WP05](../../wp05-mysql-at-plan.md)、[契约](../../mysql-commit-direction-contract.md)和 [Java 文件映射](../../mysql-commit-direction-file-map.csv)。首期范围包含显式003迁移、原身份保护、重复／并发回调、真实锁等待／取消／期限、权限回滚及实际进程恢复。

```mermaid
flowchart LR
    A[正式提交回调] --> B[文件 Pending]
    B --> C[持久提交授权]
    C --> D[同事务清理原日志并写终态]
    D --> E[实际 COMMIT 确认]
    E --> F[文件 Completed]
    D --> G[故障保留 Pending]
    G --> H[原身份进程恢复]
    H --> D
```

| Rust | 普通测试 | 实际 harness | 文档测试 | 本轮 DB 执行 |
|---|---:|---:|---:|---:|
{rows}

每版资格化的53次 DB 执行由12项新增 native、8项提交前仲裁、10项准备、18项回调／受管理回归、4种故障及1个固定原 Java TC 场景组成；新增 native 具有38个具体观察。native 的受控 TCP 对端与真实 Java TC 场景分别记录。四种故障为提交请求阻断、确认丢失、确认延迟和终态语句阻断，均由实际数据库、原报文、文件日志和独立子进程核验。权限场景保留 MySQL 预编译事件的空 SQL_TEXT，以实际 DELETE 计数、同线程先后事件及回滚／恢复事实验收，不冒称提供该场景的原包 SQL 文本。

双版累计 {result['wire_negative_controls']} 项原包／日志反例、{result['role_negative_controls']} 项角色反例及 {result['missing_member_negative_controls']} 项必需成员缺失反例均拒绝。185个依赖候选与150个实际编译依赖保持原固定闭包；fmt、Clippy警告即错误、文档警告即错误及完整测试通过。固定原 JAR 和原 Java TC 已运行，新编译内容为认证测试夹具；原 JUnit 尚未执行。一次1.85终态阻断测试虽通过数据库断言，但授权ACK转发后的记账时间戳与下一COMMIT接收出现倒序，被原有严格报文验收拒绝；未放宽检查，仅重跑该角色，拒绝原包与退出记录保留且不计入通过数量。

14列恢复表须先停旧写入器再显式迁移；原七列 Undo 不变，旧行版本及原字段保留，新读取器继续兼容11／12列。14列的旧 phase1 行仍须新正式提交回调授权。真实旧程序拒绝扩展表且不修改 Pending；原回滚路径仍受回归验证。完整 AsyncWorker 队列／批调度、189份原Java测试源码及1135个静态测试方法声明的完整迁移仍未完成。

原 TC 排他锁覆盖风险未接受，共享锁按用户要求暂缓；业务提交未知恢复、任意旧尝试围栏、生产认证／TLS、多节点和 GC 仍未验收。本轮隔离夹具通过不关闭这些门槛。

证据根：[{packet.name}]({packet.name}/result.json)，catalog SHA-256 `{digest}`；{result['source_inputs']}个输入、{len(members)}个成员。[独立只读审查]({review_path.name})无未解决P1/P2。[全部记录尝试的运行审计](../mysql-commit-direction/{audit_path.name})确认{audit['owned_containers_verified_absent']}个数据库容器不存在、{audit['owned_java_runs_verified_stopped']}组Java运行停止，D盘现场保留；只读状态查询的额外诊断容器另有独立退出审计，不计资格化实例。[机器状态](../../wp05-mysql-commit-direction.json)保留全部开放项。
"""
    readme=base/'README.md';write(readme,text)
    publication=dict(evidence=rel(packet),catalog_sha256=digest,selected_scope_only=True,files=[dict(path=rel(p),sha256=sha(p)) for p in [state_path,readme,review_path,audit_path,ROOT/'docs/phase-2/mysql-commit-direction-contract.md',ROOT/'docs/phase-2/mysql-commit-direction-file-map.csv',ROOT/'tools/formal-commit-direction-gate/scope-refinement-20261117.json',ROOT/'docs/phase-2/evidence/mysql-commit-direction/status-diagnostic-audit-20261117.json']])
    publication_path=base/'publication-20261117.json';write(publication_path,publication)
    progress=ROOT/'docs/delivery-progress.md';marker='## 2026-11-17：正式提交回调的持久清理'
    if marker not in progress.read_text('utf-8'):
        with progress.open('a',encoding='utf-8') as f:f.write(f"\n\n{marker}\n\n- 显式003能力迁移、正式回调持久授权、同事务原日志清理／终态、提交未知后的原身份恢复已按首期契约验证；不提供公开业务COMMIT。\n- 双Rust各339普通／77实际harness／4文档测试，各53次DB执行，新增native38个具体观察。原包反例{result['wire_negative_controls']}、角色反例{result['role_negative_controls']}、成员缺失反例{result['missing_member_negative_controls']}全部拒绝；185依赖候选／150实际编译闭包未升级。\n- [最终验收](phase-2/evidence/mysql-commit-direction-gate/README.md)及[机器状态](phase-2/wp05-mysql-commit-direction.json)已发布；root {packet.name}，catalog {digest}，{result['source_inputs']}输入／{len(members)}成员；独立只读终核无剩余确认P1/P2。\n- 原业务COMMIT／未知业务提交恢复、任意旧尝试围栏、原TC排他锁覆盖、完整AsyncWorker／Java源测试资产迁移、WP05／WP06／V1、生产TLS／多节点／GC仍OPEN；共享锁暂缓，外部TC风险未接受。\n")
    print(json.dumps(dict(state=rel(state_path),publication=rel(publication_path),publication_sha256=sha(publication_path)),ensure_ascii=False),flush=True)
if __name__=='__main__':run(*map(Path,sys.argv[1:]))
