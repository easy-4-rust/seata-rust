"""Publish the original76 case floor while retaining TableMeta and full-module completion gates."""
import argparse
import dataclasses
import json
import re
import shutil
import sys
from pathlib import Path
from freeze import ROOT,DOC,MODULE
from qualify import read,sha
from verify import need

GATE=ROOT/'docs/phase-2/evidence/sqlparser-table-gate/run_20261118_105502_3c628c06'
DIGEST='84080e6737b775669c064aed2d792a191ba16da54731125976114792e779d802'
MAPPING=dict(zip([
    'testSetGetTableName','testSetGetCaseSensitive','testGetColumnMeta','testGetAllColumns','testGetAllIndexes',
    'testGetPrimaryKeyMap','testGetPrimaryKeyMapNoPrimaryKey','testGetCaseInsensitivePKs','testGetCaseInsensitivePKsNoPrimaryKey',
    'testGetPrimaryKeyOnlyName','testGetOnUpdateColumnsOnlyName','testGetOnUpdateColumnsOnlyNameNoUpdates',
    'testContainsPKWithNullList','testContainsPKWithNoPrimaryKey','testContainsPKWithExactMatch','testContainsPKWithCaseInsensitiveMatch',
    'testContainsPKWithNoMatch','testEquals','testHashCode'],[
    'set_get_table_name','set_get_case_sensitive','get_column_meta','get_all_columns','get_all_indexes',
    'get_primary_key_map','get_primary_key_map_no_primary_key','get_case_insensitive_pks','get_case_insensitive_pks_no_primary_key',
    'get_primary_key_only_name','get_on_update_columns_only_name','get_on_update_columns_only_name_no_updates',
    'contains_pk_with_null_list','contains_pk_with_no_primary_key','contains_pk_with_exact_match','contains_pk_with_case_insensitive_match',
    'contains_pk_with_no_match','equals','hash_code']))


def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n','utf-8')


def frozen():
    need(sha(GATE/'catalog.json')==DIGEST,'Frozen catalog drift')
    for row in read(GATE/'catalog.json')['members']:need(sha(GATE/row['path'])==row['sha256'],'Frozen member drift')
    for row in read(GATE/'verification/inputs.json'):
        if row['path'].startswith('crates/') or row['path'] in {'Cargo.toml','Cargo.lock'}:need(sha(ROOT/row['path'])==row['sha256'],'Verified source drift')


def audit_current():
    sys.path.insert(0,'C:/Users/hnxyh/.agents/skills/rust-java-migration-testing/scripts')
    import audit_migration_tests as audit
    originals=[dataclasses.replace(item,file=Path(item.file).as_posix()) for item in audit.extract_java_tests(MODULE)]
    assets=audit.discover_java_test_assets(MODULE,[])
    parity=audit.validate_source_parity_manifest(DOC/'source-test-parity.json',MODULE,ROOT,originals,assets)
    ledger=audit.parse_object_ledger(DOC/'对象级对照表.md')
    need(len(originals)==parity.mapped_java_tests==76 and len(assets)==0,'Original denominator differs')
    need(ledger.rows_scanned==31 and ledger.state_counts['IMPLEMENTED']==13 and ledger.state_counts['PARTIAL']==1 and ledger.state_counts['MISSING']==17,'Object denominator differs')
    need(ledger.migration_completion_blocked,'Partial/missing objects must block full module')
    need(not any('unsafe manifest path' in e or 'source test was not found' in e for e in parity.errors),'Structural manifest error')
    inventory=read(DOC/'inventory.json');manifest=read(DOC/'source-test-parity.json')
    need(sum(r['state']=='MIRRORED' and r['differential']=='MATCH' for r in inventory['cases'])==76,'Inventory floor differs')
    need(sum(r['disposition']=='MIRRORED' and r['result_parity']=='MATCH' for r in manifest['source_tests'])==76,'Manifest floor differs')
    need(manifest['acceptance_module']['scoped_test_execution']['ordinary_passed']==131 and manifest['runs']['rust']['skipped']==0,'Acceptance scope confused with global ignored tests')
    for row in inventory['objects']:
        if row['state'] in {'IMPLEMENTED','PARTIAL'}:need(sha(ROOT/row['rust'])==row['rust_sha256'],'Object bytes drift')
    for row in read(DOC/'verification/table-evidence-aliases.json')['aliases']:need(sha(DOC/row['path'])==row['sha256']==sha(ROOT/row['origin']),'Evidence alias drift')
    documents=[]
    for p in DOC.glob('*.md'):
        current=p.read_text('utf-8').split('<!-- historical-design-appendix-start -->',1)[0]
        need('13 IMPLEMENTED、1 PARTIAL、17 MISSING' in current and '76 MATCH、0 MISSING' in current and 'TableMeta仍为PARTIAL' in current,'Current document state differs')
        need('57 MATCH' not in current and '19 MISSING' not in current and '各440' not in current,'Stale current facts')
        documents.append(dict(path=p.relative_to(ROOT).as_posix(),current_nonblank_lines=sum(bool(s.strip()) for s in current.splitlines())))
    result=dict(status='ORIGINAL76_CASE_FLOOR_PASSED_FULL_MODULE_CORRECTLY_BLOCKED',objects=dataclasses.asdict(ledger),source_parity=dataclasses.asdict(parity),documents=documents,
        full_core_complete=False,source_case_floor_is_not_full_object_coverage=True,policy_override='User requires snake_case seata_test')
    target=GATE.parent/'current-module-final-audit-20261118.json';need(not target.exists(),'Do not overwrite audit');save(target,result)
    print(json.dumps(dict(status=result['status'],source_MATCH=76,TableMeta='PARTIAL',parity_errors=parity.errors)),flush=True)


def publish():
    frozen();result=read(GATE/'result.json');inventory=read(DOC/'inventory.json');parity=read(DOC/'source-test-parity.json')
    aliases=[]
    names={'result.json':'table-result.json','commands.json':'table-commands.json','1.99-source-diff.json':'table-source-diff.json',
        '1.99-table-hash-diff.json':'table-hash-diff.json','1.99-table-edge-diff.json':'table-edge-diff.json',
        '1.99-column-diff.json':'table-column-edge-diff.json','1.99-index-diff.json':'table-index-edge-diff.json',
        '1.99-acceptance.log':'table-acceptance-package.log'}
    for source,name in names.items():
        origin=GATE/'verification'/source;target=DOC/'verification'/name
        need(not target.exists(),'Do not overwrite evidence alias');shutil.copy2(origin,target)
        aliases.append(dict(path=target.relative_to(DOC).as_posix(),origin=origin.relative_to(ROOT).as_posix(),sha256=sha(target)))
    for source,name in [('oracle/commands.json','table-java-commands.json'),('comparison-counterexamples.json','table-comparison-counterexamples.json')]:
        origin=GATE/source;target=DOC/'verification'/name;need(not target.exists(),'Do not overwrite alias');shutil.copy2(origin,target)
        aliases.append(dict(path=target.relative_to(DOC).as_posix(),origin=origin.relative_to(ROOT).as_posix(),sha256=sha(target)))
    save(DOC/'verification/table-evidence-aliases.json',dict(aliases=aliases))
    inventory.update(current_gate='../evidence/sqlparser-table-gate/'+GATE.name,current_catalog_sha256=DIGEST,source_cases_MATCH=76,source_cases_MISSING=0,
        implementation_in_progress='Original76 case floor qualified; TableMeta remains PARTIAL and17 source objects MISSING')
    for row in inventory['objects']:
        if row['state'] in {'IMPLEMENTED','PARTIAL'}:
            row['rust_sha256']=sha(ROOT/row['rust']);row['evidence']='verification/table-result.json'
        if row['state']=='PARTIAL':row['verification_level']='ORIGINAL19_MATCH_WITH_PUBLIC_API_GAPS'
    for row in inventory['cases']:
        if 'TableMetaTest#' in row['id']:row['target']='crates/seata_test/tests/table_meta.rs#'+MAPPING[row['id'].split('#')[1]]
        row.update(state='MIRRORED',differential='MATCH',evidence='verification/table-source-diff.json')
    for name in ['MetadataLocale','MetadataMapValue','MetadataEntryCursor','PrimaryKeyMap']:
        if name not in inventory['Rust_extensions']:inventory['Rust_extensions'].append(name)
    inventory['external_dependency_objects_partial']=[dict(java_type='org.apache.seata.common.util.LowerCaseLinkHashMap',rust='crates/seata_sqlparser/src/util/lower_case_link_hash_map.rs',state='PARTIAL',reason='Full Map views/default methods absent; not in31 SQL parser source denominator')]
    save(DOC/'inventory.json',inventory)
    for row in parity['source_tests']:
        if 'TableMetaTest.java#' in row['source']:
            row['targets']=['crates/seata_test/tests/table_meta.rs#'+MAPPING[row['source'].split('#')[1]]]
        row.update(disposition='MIRRORED',result_parity='MATCH',evidence='verification/table-source-diff.json')
        for key in ['contract_preserved','inputs_preserved','assertions_preserved','fixture_state_preserved','cleanup_preserved','case_expansion_complete']:row[key]=True
    parity['acceptance_module'].update(status='PASS',not_run=0,artifact='verification/table-acceptance-package.log',
        scoped_test_execution=dict(status='PASS',ordinary_passed=131,failed=0,skipped=0),
        scope='131 package tests=76 original source cases+55 Rust obligations; one private core cursor unit is separate; full object coverage remains blocked')
    parity['runs']['java'].update(command='New original SourceJUnitRunner76 plus four peers; exact argv in table-java-commands.json',artifact='verification/table-java-commands.json')
    parity['runs']['rust'].update(command='cargo test --offline --locked -p seata-test --all-targets; both1.99/1.85',status='PASS',skipped=0,not_run=0,artifact='verification/table-acceptance-package.log')
    parity['runs']['differential'].update(command='python tools/formal-sqlparser-core/verify_table.py --oracle '+result['oracle'],status='PASS',matched=76,not_run=0,artifact='verification/table-source-diff.json')
    parity['whole_workspace_regression']=dict(ordinary=471,harness=95,ignored=143,doc=4,toolchains=['1.99','1.85'],artifact='verification/table-commands.json',scope='Separate full workspace; ignored143 are not ignored acceptance-package tests')
    parity['inherited_original_JUnit_76_not_rerun']=False
    parity['dependency_obligation_history'].append(parity['rust_obligation'])
    parity['rust_obligation']=dict(new_test_count=12,prior_package_obligations=43,package_obligations=55,private_core_unit_tests=1,
        targets=['crates/seata_test/tests/table_exception_hierarchy.rs','crates/seata_test/tests/table_java_diff.rs','crates/seata_test/tests/table_map_obligations.rs'],
        computed_primary_hash_order_records=283,computed_table_edge_records=12,identity_inputs_not_MATCH=8,
        artifact='verification/table-result.json',obligations=['Shared maps/locale; live future values and exact cursor version points','Callback reentry and Drop after unlock; tail scalar reread',
            'Original HashMap tree bins/root front/resize split/null and raw UTF16 names','Throwable actual parent chain and identity; Map.equals catch families','containsPK requires every key and preserves null elements'])
    for name,count in [('primary-hash-java.tsv',283),('table-edges-java-v2.tsv',12)]:
        p=ROOT/'crates/seata_test/tests/fixtures/sqlparser_core'/name
        parity['generated_Rust_oracle_assets'].append(dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p),computed_records=count,
            source=(GATE/'oracle'/('table-edges-java.tsv' if name.endswith('v2.tsv') else name)).relative_to(ROOT).as_posix(),
            purpose='Generated oracle, not original source asset; enum hashes and throwable descriptors are INPUT only'))
    save(DOC/'source-test-parity.json',parity)
    matrix='\n'.join('| '+r['java_type']+' | `'+r['rust']+'` | '+r['state']+' |' for r in inventory['objects'])
    notes={
        '迁移路线图.md':'下一步按已有WP05继续默认Locale引导、Map可变视图和转义/SPI链；不得以19例通过关闭TableMeta。',
        '对象级对照表.md':'每个IMPLEMENTED对象均有真实源/目标/证据；TableMeta本体已有实现但仍缺公开路径，不能晋升IMPLEMENTED。',
        '语义迁移对照表.md':'保留原equals方向、nullable、ALL主键条件、caseSensitive哈希不一致及每次调用的语言上下文。',
        '对象名称一致性检查.md':'保留原类型名、最后一级Java子包与snake_case文件；lib.rs/mod.rs仅声明和重导出。'}
    for p in DOC.glob('*.md'):
        old=p.read_text('utf-8');title=old.splitlines()[0];_,marker,history=old.partition('<!-- historical-design-appendix-start -->')
        current=f'''{title}

## 当前规格与边界

- 规格事实源：`../wp05-mysql-at-plan.md`、`../source-junit-contract.md`；WP05 IMPLEMENTING。
- Java基线：`cba73e180fd8896a99df6415b859d275122feaa7`；Rust基线：`130ad634279cef10cd2998fc3bfae82996224d66`。
- Brownfield；保留当前分支及原未提交修改，不初始化另一套SDD。
- 固定分母：31对象、12原测试文件、76原案例、0原外部测试资产。
- 源对象：13 IMPLEMENTED、1 PARTIAL、17 MISSING。
- 原案例：76 MATCH、0 MISSING；原案例通过不代表完整公开API覆盖。
- TableMeta仍为PARTIAL：默认Locale引导、escape/SPI、SqlServer子类型桥和完整Map视图尚未完成。
- Rust目标`crates/seata_sqlparser`；独立验收包`crates/seata_test`，publish=false。
- 目录retain_segments=1，按用户规则保留struct/util子包；扩展及外部依赖不计31对象分子。
- {notes[p.name]}

## 对象级对照矩阵

| Java对象 | Rust路径 | 状态 |
|---|---|---|
{matrix}

## 本轮实现与原语义

- TableMeta共享表名、原表名、caseSensitive、列Map和索引Map；Clone保留Java句柄共享。
- 列/索引Map按当前显式共享Locale规范化键，替换保留插入位置，允许null值，null插入键抛原NPE。
- LowerCaseLinkHashMap位于util子包，只实现当前真实依赖路径；完整Map接口仍PARTIAL，不计SQL parser源对象。
- 主键Map保留JDK17分桶、扩容、树化、树根前移、分裂/退化和nullable键；不能用排序Map替代。
- 主键集合保留非传递JDK忽略大小写比较器的红黑树形状及首拼写，不以排序数组替代。
- containsPK按原containsAll要求全部主键；精确匹配优先，大写转换保留null集合元素。
- 原equals忽略originalTableName/caseSensitive；原hashCode仍包含caseSensitive，保留此不一致。
- Map.equals使用原目标键和原调用方向；捕获NPE/CCE及其实际子类，不吞CME或同名无关异常。
- 自定义异常桥显式传入实际父类链，保留原类、原始UTF16消息、原因与共享身份。
- 用户回调和用户值Drop均在Map/表锁外；读取未来替换值，尾部标量按原时点重读。
- 迭代器next前检查结构版本，最后hasNext不增加原实现没有的CME检查。
- 无主键异常保留原NotSupportYetException类及nullable/原始UTF16表名消息。
- ColumnMeta为608行单对象内聚例外；其余文件符合尺寸规则，未新增依赖。

## 实际验证证据

| 边界 | 结果 | 证据 |
|---|---|---|
| 原Java JUnit | 本轮新运行76通过，13事件反例拒绝 | `verification/table-java-commands.json` |
| 原案例差分 | 76 MATCH、0 MISSING=23基础+18列+16索引测试+19表 | `verification/table-source-diff.json` |
| 双工具链 | 1.99/1.85各471普通测试/95 harness/4doc，143 ignored | `verification/table-commands.json` |
| 显式验收包 | 131通过=76原案例+55 Rust补充；0失败/0忽略 | `verification/table-acceptance-package.log` |
| 主键HashMap顺序 | 283条真实Java前缀计算结果MATCH | `verification/table-hash-diff.json` |
| 表元数据边界 | 12条真实Java计算结果MATCH | `verification/table-edge-diff.json` |
| 继承计算观察 | 23,535组回归MATCH；另12,384组三字段Locale观察MATCH | `verification/table-result.json` |
| 错误变体 | 6个编译后运行时失败；比较器12反例拒绝 | `verification/table-result.json` |
| 冻结证据 | 642成员/523验证输入 | `{DIGEST}` |

- 两组独立JVM枚举身份各4个仅计INPUT；异常类/父类描述也仅计输入，不能伪称计算结果。
- 2,799简单字符映射、5,004大小写属性与3词边界/词典文件仅为兼容性INPUT。
- 保留原ROOT展开、Σ条件、tr/az/lt及ROOT/th随机边界查询/缓存；12,384组三字段为有限语料验证。
- 继承1个未采用的预计算Locale变体仍未分类存活，未证明等价；353,922次有界搜索不构成证明。
- 219原运行文件及5实际JDK文件、31源对象、12原测试副本与实际生产者字节均绑定。
- E0432只作结构RED；异常继承捕获另有真实运行RED，修复后GREEN。
- 静态布局17个缺失对象、0未允许警告；PARTIAL对象另列，不伪称完整布局或全语义通过。
- 核心私有游标单测另1项，不扩充76原案例或验收包131计数。

## 尚未完成与下一步

- 默认OS/进程Locale读取和管理仍缺；当前显式注入语言，不能宣称默认构造全兼容。
- getEscapePkNameList/ColumnUtils/EscapeHandler/SPI链仍缺；SqlServer子类型equals桥仍缺。
- 返回Map的完整可变Entry/Key/Value视图、默认方法和主键Map删除路径仍缺。
- 17原源对象仍MISSING；TableMeta仍为PARTIAL，完整SQL parser core继续OPEN。
- 全局1135源方法中已对齐76、仍1059；1327具体案例中已对齐76、仍1251。
- 143环境测试仍忽略，无新增数据库现场认证，AT SQL支持矩阵不扩大。
- 共享锁按用户延期；外部TC风险未接受；WP05十四项、WP06、全V1及TC安全保持OPEN。
- 当前机器状态`../wp05-sqlparser-table.json`；历史快照只作历史记录，不作为当前完成声明。

'''
        p.write_text(current+marker+history,'utf-8')
    state=result|dict(gate=GATE.relative_to(ROOT).as_posix(),catalog_sha256=DIGEST,package_test_execution_passed=131,
        selected_source_methods_MATCH=76,selected_source_methods_MISSING=1059,selected_source_concrete_cases_MATCH=76,selected_source_concrete_cases_MISSING=1251,
        current_inventory='docs/phase-2/sqlparser_core/inventory.json',current_source_parity='docs/phase-2/sqlparser_core/source-test-parity.json',
        current_specification_state='WP05_IMPLEMENTING_ORIGINAL76_TEST_FLOOR_PASSED_CORE_OBJECTS_OPEN')
    save(ROOT/'docs/phase-2/wp05-sqlparser-table.json',state)
    (GATE.parent/'README.md').write_text('# TableMeta首期实现及原案例差分\n\n'
        '原76案例双工具链全部MATCH；源对象仍13 IMPLEMENTED、1 PARTIAL、17 MISSING。TableMeta不宣称完整迁移。\n\n'
        '- 已实现共享元数据、显式Locale、主键/更新列及原equals/hashCode行为；283条主键顺序和12条表边界计算结果MATCH。\n'
        '- 原JUnit76本轮新运行；Rust1.99/1.85各471普通测试/95 harness/4doc通过，143 ignored。显式验收包131通过、0忽略。\n'
        '- 6错误变体运行时失败，12比较反例拒绝；继承Locale存活变体未分类，不计拒绝且未证明等价。\n'
        '- 默认Locale引导、转义/SPI、SqlServer桥和完整Map视图未完成；全部core/WP05/WP06/V1与TC安全保持OPEN。\n\n'
        f'root `{GATE.name}`；catalog `{DIGEST}`；642成员/523输入。\n\n'
        '[机器状态](../../wp05-sqlparser-table.json) · [完整对象清单](../../sqlparser_core/对象级对照表.md)\n','utf-8')
    progress=ROOT/'docs/delivery-progress.md';old=progress.read_text('utf-8');title='## 2026-11-18：TableMeta首期实现与原76案例差分'
    need(title not in old,'Already published');progress.write_text(old+'\n\n'+title+'\n\n'
        '- 原76案例双工具链MATCH；原Java76本轮新运行，283主键顺序/12表边界匹配。共享映射、重入、异常继承已实现。\n'
        '- Rust1.99/1.85各471测试/95 harness/4doc，143 ignored；验收包131通过/0忽略；6运行变体拒绝、12比较负控拒绝。\n'
        '- [证据](phase-2/evidence/sqlparser-table-gate/README.md)和[机器状态](phase-2/wp05-sqlparser-table.json)。对象13完整/1部分/17缺失；默认Locale、SPI和完整Map继续实施，全部交付保持OPEN。\n','utf-8')
    p=ROOT/'README.md';old=p.read_text('utf-8');p.write_text(old+'\n\n[TableMeta首期差分](docs/phase-2/evidence/sqlparser-table-gate/README.md)：原76案例双工具链MATCH，TableMeta仍PARTIAL，源对象13完整/1部分/17缺失；完整交付继续OPEN。\n','utf-8')
    audit_current()


def finalize():
    frozen();family=GATE.parent;review=read(family/'readonly-review-20261118.json')
    need(review['confirmed_remaining_P1']==review['confirmed_remaining_P2']==0 and review['mode']=='READ_ONLY','Review remains unresolved')
    need(read(family/'current-module-final-audit-20261118.json')['full_core_complete'] is False,'Module gate incorrectly closed')
    files=[ROOT/'README.md',ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/delivery-progress.md',ROOT/'docs/phase-2/wp05-sqlparser-table.json',family/'README.md',family/'readonly-review-20261118.json',family/'current-module-final-audit-20261118.json']
    for directory in [DOC,ROOT/'tools/formal-sqlparser-core',ROOT/'crates/seata_sqlparser',ROOT/'crates/seata_test']:
        files.extend(p for p in directory.rglob('*') if p.is_file())
    publication=dict(status='ORIGINAL76_CASE_FLOOR_PUBLISHED_FULL_GOAL_OPEN',gate=GATE.relative_to(ROOT).as_posix(),catalog_sha256=DIGEST,
        members=642,verification_inputs=523,source_object_states=dict(IMPLEMENTED=13,PARTIAL=1,MISSING=17),original_case_states=dict(MATCH=76,MISSING=0),
        package_tests_passed=131,full_core_complete=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,TC_safety_gate_passed=False,
        external_TC_risk_accepted=False,post_gate_changes='Current documentation/ledgers and publication tools; verified production/test bytes unchanged',
        files=[dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in sorted(set(files))])
    target=family/'publication-20261118.json';need(not target.exists(),'Do not overwrite publication');save(target,publication)
    for row in publication['files']:need(sha(ROOT/row['path'])==row['sha256'],'Publication drift')
    print(json.dumps(dict(publication=target.relative_to(ROOT).as_posix(),sha256=sha(target),files=len(publication['files']),source_MATCH=76,full_core_complete=False)),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('stage',choices=['publish','finalize']);args=p.parse_args();publish() if args.stage=='publish' else finalize()
