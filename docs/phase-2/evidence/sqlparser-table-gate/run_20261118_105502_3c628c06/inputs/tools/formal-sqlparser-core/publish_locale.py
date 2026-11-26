"""Publish current Locale evidence without promoting dependencies into source-object completion."""
import argparse
import dataclasses
import json
import re
import sys
from pathlib import Path
from freeze import ROOT,DOC,MODULE
from qualify import read,sha
from verify import need

GATE=ROOT/'docs/phase-2/evidence/sqlparser-locale-gate/run_20261118_052535_fb6c7ec8'
DIGEST='c411a512731f65f154c5767bbb3b015af1ee14156537497eed4c4ab232cb34e9'


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
    assets=audit.discover_java_test_assets(MODULE,[]);parity=audit.validate_source_parity_manifest(DOC/'source-test-parity.json',MODULE,ROOT,originals,assets)
    ledger=audit.parse_object_ledger(DOC/'对象级对照表.md')
    need(len(originals)==parity.mapped_java_tests==76 and len(assets)==0,'Original denominator differs')
    need(ledger.rows_scanned==31 and ledger.state_counts['IMPLEMENTED']==13 and ledger.state_counts['MISSING']==18 and ledger.state_counts['PARTIAL']==0,'Source object denominator differs')
    need(ledger.migration_completion_blocked and parity.migration_completion_blocked,'Missing source work must block completion')
    need(not any('unsafe manifest path' in e or 'source test was not found' in e for e in parity.errors),'Structural manifest error')
    manifest=read(DOC/'source-test-parity.json');rows=manifest['source_tests']
    need(sum(row['disposition']=='MIRRORED' and row['result_parity']=='MATCH' for row in rows)==57 and sum(row['disposition']=='MISSING' for row in rows)==19,'Source case split differs')
    need(manifest['acceptance_module']['scoped_test_execution']['ordinary_passed']==100,'Explicit package scope differs')
    for row in read(DOC/'verification/locale-evidence-aliases.json')['aliases']:need(sha(DOC/row['path'])==row['sha256']==sha(ROOT/row['origin']),'Current evidence alias drift')
    documents=[]
    for path in DOC.glob('*.md'):
        current=path.read_text('utf-8').split('<!-- historical-design-appendix-start -->',1)[0]
        need('13 IMPLEMENTED、18 MISSING' in current and '57 MATCH、19 MISSING' in current and
             '12,384组' in current and '未分类存活' in current and 'TableMeta本体仍为MISSING' in current,'Current facts insufficient')
        documents.append(dict(path=path.relative_to(ROOT).as_posix(),nonblank_current_lines=sum(bool(line.strip()) for line in current.splitlines())))
    result=dict(status='FULL_MODULE_COMPLETION_CORRECTLY_BLOCKED',objects=dataclasses.asdict(ledger),source_parity=dataclasses.asdict(parity),current_documents=documents,
        note='13/31 objects and57/76 original cases; dependencies and private unit do not promote TableMeta or count as original cases')
    target=GATE.parent/'current-module-final-audit-20261118.json';need(not target.exists(),'Do not overwrite audit');save(target,result)


def publish():
    frozen();result=read(GATE/'result.json');inventory=read(DOC/'inventory.json')
    need(inventory['current_catalog_sha256']=='9df62e28460c66b2aae94071d417f2f10ce3c2a46eedf02af237b3daeb6845bb','Current ledger advanced')
    aliases=[]
    for source,name in [(GATE/'result.json','locale-result.json'),(GATE/'verification/commands.json','locale-commands.json'),
        (GATE/'verification/1.99-source-diff.json','locale-source-diff.json'),(GATE/'verification/1.99-locale-casing-diff.json','locale-casing-diff.json'),
        (GATE/'verification/1.99-locale-boundary-diff.json','locale-boundary-diff.json'),(GATE/'oracle/commands.json','locale-java-commands.json'),
        (GATE.parent/'explicit-acceptance-package-20261118.log','locale-acceptance-package.log')]:
        target=DOC/'verification'/name;need(not target.exists(),'Do not overwrite evidence alias');target.write_bytes(source.read_bytes())
        aliases.append(dict(path=target.relative_to(DOC).as_posix(),sha256=sha(target),origin=source.relative_to(ROOT).as_posix()))
    save(DOC/'verification/locale-evidence-aliases.json',dict(aliases=aliases))
    need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',(DOC/'verification/locale-acceptance-package.log').read_text('utf-8'))))==100,'Package count differs')
    inventory.update(current_gate='../evidence/sqlparser-locale-gate/'+GATE.name,current_catalog_sha256=DIGEST)
    need('JdkCasing' not in inventory['Rust_extensions'],'Extension already published');inventory['Rust_extensions'].append('JdkCasing')
    inventory['Rust_private_dependencies']=['CompatibilityDataReader','JdkCharacterCategories','JdkWordDictionary','JdkWordProfile','Utf16Cursor']
    for name in ['jdk17_string_casing_input.bin','jdk17_word_root_input.bin','jdk17_word_th_input.bin','jdk17_word_th_dictionary_input.bin']:
        path='crates/seata_sqlparser/src/data/'+name
        inventory['compatibility_inputs'].append(dict(path=path,sha256=sha(ROOT/path),role='INPUT_NOT_RESULT',
            record_count=5004 if name=='jdk17_string_casing_input.bin' else None,source='Actual fixed JDK17.0.12, not host Rust Unicode'))
    for row in inventory['objects']:
        if row['state']=='IMPLEMENTED':need(sha(ROOT/row['rust'])==row['rust_sha256'],'Original object source drift');row['evidence']='verification/locale-result.json'
    for row in inventory['cases']:
        if row['state']=='MIRRORED':row['evidence']='verification/locale-source-diff.json'
    save(DOC/'inventory.json',inventory)
    parity=read(DOC/'source-test-parity.json')
    for row in parity['source_tests']:
        if row['disposition']=='MIRRORED':row['evidence']='verification/locale-source-diff.json'
    parity['acceptance_module'].update(artifact='verification/locale-acceptance-package.log',status='PARTIAL',not_run=19,
        scoped_test_execution=dict(status='PASS',ordinary_passed=100,failed=0,skipped=0),
        scope='100 package tests=57 original cases+43 Rust obligations; one private core cursor unit is separate;19 original cases remain MISSING')
    parity['runs']['rust'].update(artifact='verification/locale-commands.json',command='24 actual dual-toolchain,8 rejected defects and1 unclassified surviving variant commands')
    parity['runs']['differential'].update(artifact='verification/locale-source-diff.json',command='python tools/formal-sqlparser-core/verify_locale.py --oracle '+result['oracle'])
    parity['generated_Rust_oracle_assets'].append(dict(path='crates/seata_test/tests/fixtures/sqlparser_core/jdk17-locale-observations.tsv',
        sha256=sha(ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/jdk17-locale-observations.tsv'),source=(GATE/'oracle/jdk17-locale-observations.tsv').relative_to(ROOT).as_posix(),
        computed_context_records=12384,result_fields_per_record=3,purpose='Computed lower/upper/isBoundary contexts; not original source JUnit cases'))
    parity['dependency_obligation_history'].append(parity['rust_obligation'])
    parity['rust_obligation']=dict(new_test_count=3,prior_test_count=41,integration_tests=2,private_unit_tests=1,
        targets=['crates/seata_test/tests/locale_casing.rs','crates/seata_sqlparser/src/utf16_cursor.rs#tests::previous_preserves_code_point_before_partial_surrogate_position'],
        computed_context_records=12384,result_fields_per_record=3,compatibility_property_inputs_not_MATCH=5004,
        word_profile_input_files_not_MATCH=3,artifact='verification/locale-casing-diff.json',
        obligations=['Raw UTF16,ROOT expansion,Greek final Sigma andtr/az/lt conditions',
            'Original random-access following/previous-safe/caches andThai dictionary backtracking',
            'Code point before a position inside a surrogate pair preserves original high code unit'])
    parity['surviving_variant']=dict(name='precompute-final-sigma-boundaries',status='SURVIVED_NO_COUNTEREXAMPLE_FOUND',equivalence_proven=False,
        bounded_actual_JDK_searches=353922,counted_as_rejected_defect=False,adopted_in_production=False)
    save(DOC/'source-test-parity.json',parity)
    notes=[
        '新增JdkCasing显式语言上下文转换；TableMeta本体仍为MISSING，默认系统Locale读取、管理和元数据Map集成仍待完成。',
        '12,384组上下文的lower/upper/词边界三个计算字段双工具链全部MATCH；5,004属性记录及3词边界/词典文件只计INPUT。',
        '保留原ROOT多码点展开、Σ条件、tr/az/lt规则、ROOT/th分词配置，以及原随机查询、回退和缓存顺序。',
        '压缩补充平面表合法同起点空区间和随机查询边界均曾产生实际运行RED，已修复；未用宿主Unicode算法替代。',
        '8错误变体实际运行失败；另1预计算isBoundary变体为未分类存活，不计负控通过，未采用，未证明全域等价。',
        '353,922次原JDK有界搜索未找到该变体反例；有限搜索不是等价证明，也不计原Seata测试覆盖。',
        '原getPrevious在代理对中间回退实际返回55296/位置1；私有游标1项回归另列，不扩充76原案例分母。',
        'Rust1.99/1.85各440普通测试、91 harness、4doc通过；143 ignored。验收包100=57源案例+43补充测试，核心私有单测另1项。',
        'MetadataError新增InvalidCompatibilityData报告固定输入损坏；这是Rust扩展错误类别，不能伪造Java业务异常。',
        '当前仍13 IMPLEMENTED、18 MISSING和57 MATCH、19 MISSING；全部core/WP05/WP06/V1及TC安全保持OPEN。']
    for path in DOC.glob('*.md'):
        current,marker,history=path.read_text('utf-8').partition('<!-- historical-design-appendix-start -->')
        current=current.replace('各437普通测试/90 harness/4doc','各440普通测试/91 harness/4doc').replace('98通过=57源案例+41 Rust补充测试','100通过=57源案例+43 Rust补充测试')
        current=current.replace('本依赖7组真实行为失败，8比较反例拒绝','8组真实行为失败＋1未分类存活；12比较反例拒绝').replace('本依赖7组隔离mutant均为编译后实际行为失败','8组错误变体为编译后实际行为失败，另1存活变体未计通过')
        current=current.replace('verification/table-set-result.json','verification/locale-result.json').replace('verification/table-set-commands.json','verification/locale-commands.json')
        current=current.replace('verification/table-set-source-diff.json','verification/locale-source-diff.json').replace('verification/table-set-acceptance-package.log','verification/locale-acceptance-package.log')
        current=current.replace('537成员/468输入','596成员/497输入').replace('9df62e28460c66b2aae94071d417f2f10ce3c2a46eedf02af237b3daeb6845bb',DIGEST)
        current=current.replace('| 新计算观察 | 144 MATCH','| 已有索引计算观察 | 144 MATCH')
        current+='\n## Locale上下文依赖（差分已验证，存活变体未分类）\n\n'+'\n'.join('- '+note for note in notes)+'\n\n'
        path.write_text(current+marker+history,'utf-8')
    state=result|dict(gate=GATE.relative_to(ROOT).as_posix(),catalog_sha256=DIGEST,
        current_inventory='docs/phase-2/sqlparser_core/inventory.json',current_source_parity='docs/phase-2/sqlparser_core/source-test-parity.json',
        predecessor='docs/phase-2/wp05-sqlparser-table-prerequisites.json',package_test_execution_passed=100,
        selected_source_methods_MATCH=57,selected_source_methods_MISSING=1078,selected_source_concrete_cases_MATCH=57,selected_source_concrete_cases_MISSING=1270,
        current_specification_state='WP05_IMPLEMENTING_SQLPARSER_CORE_OPEN')
    save(ROOT/'docs/phase-2/wp05-sqlparser-locale.json',state)
    (GATE.parent/'README.md').write_text('# TableMeta Locale上下文依赖\n\n'
        '沿用既有WP05/ADR及完整31对象/76案例规格；TableMeta本体仍未实现，完整交付继续OPEN。\n\n'
        '- 固定JDK17上下文转换、随机词边界查询及泰语词典已实现；12,384组三字段结果双工具链MATCH。5,004属性与3词边界/词典文件仅为输入。\n'
        '- Rust1.99/1.85各440普通测试、91 harness、4doc通过，143 ignored；显式验收包100通过，核心私有游标回归另列。\n'
        '- 8错误变体运行时失败，12比较负控拒绝。另1预计算变体未分类存活，不计负控通过；353,922次原JDK有界搜索不构成等价证明。生产保留原查询顺序。\n'
        '- 原57源案例及23,535既有计算观察回归匹配；源对象仍13/31、案例仍57/76。原JUnit76为继承证据。\n'
        '- 下一步默认系统Locale管理、LowerCaseLinkHashMap及TableMeta19原案例；无新增依赖/DB现场认证，TC安全及全V1未完成。\n\n'
        f'root `{GATE.name}`；catalog `{DIGEST}`，596成员/497输入。\n\n'
        '[机器状态](../../wp05-sqlparser-locale.json) · [对象清单](../../sqlparser_core/对象级对照表.md)\n','utf-8')
    progress=ROOT/'docs/delivery-progress.md';old=progress.read_text('utf-8');title='## 2026-11-18：TableMeta Locale上下文依赖'
    need(title not in old,'Progress already published');progress.write_text(old+'\n\n'+title+'\n\n'
        '- JDK17上下文大小写及随机词边界/泰语词典实现；12,384组三字段MATCH，5,004属性及3输入文件不计结果。\n'
        '- 双工具链各440测试/91 harness/4doc，143 ignored；8运行失败＋1未分类存活变体，12比较负控拒绝。有限353,922次搜索不是等价证明。\n'
        '- [证据](phase-2/evidence/sqlparser-locale-gate/README.md)及[状态](phase-2/wp05-sqlparser-locale.json)。源码13/31、案例57/76仍未增加，继续TableMeta集成。\n','utf-8')
    readme=ROOT/'README.md';old=readme.read_text('utf-8');anchor='主对象仍待完成。';need(old.count(anchor)==1,'README anchor differs')
    readme.write_text(old.replace(anchor,anchor+' [Locale上下文依赖](docs/phase-2/evidence/sqlparser-locale-gate/README.md)已差分验证，默认Locale及表映射集成继续实施。'),'utf-8')
    audit_current();print(json.dumps(dict(source_MATCH=57,source_MISSING=19,rejected_mutants=8,unclassified_variants=1,full_core_complete=False)),flush=True)


def finalize():
    frozen();family=GATE.parent
    need(read(family/'current-module-final-audit-20261118.json')['status']=='FULL_MODULE_COMPLETION_CORRECTLY_BLOCKED','Source gap gate not retained')
    review=dict(reviewer='/root/sqlparser_values_review',mode='READ_ONLY',date='2026-11-18',confirmed_remaining_P1=0,confirmed_remaining_P2=0,
        verified=['596 catalog members/497 inputs and parent binding','Both toolchains440 ordinary/91 harness/143 ignored/4doc',
            '57 original cases,23535 inherited computed observations and12384 context triples MATCH',
            '24 actual commands;8 runtime-rejected defects and1 separately unclassified surviving variant',
            '12 comparison negative controls;5004 properties and3 word/dictionary files only INPUT',
            '219 original runtime plus5 JDK files and actual producer bindings',
            'Actual RED logs,353922 bounded actual-JDK search and private getPrevious trace; no equivalence claim'],
        full_core_complete=False,full_V1_complete=False,variant_equivalence_proven=False)
    target=family/'readonly-review-20261118.json';need(not target.exists(),'Do not overwrite review');save(target,review)
    files=[ROOT/'README.md',ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/delivery-progress.md',ROOT/'docs/phase-2/wp05-sqlparser-locale.json',
        family/'README.md',target,family/'current-module-final-audit-20261118.json',family/'explicit-acceptance-package-20261118.log']
    for directory in [DOC,ROOT/'tools/formal-sqlparser-core',ROOT/'crates/seata_sqlparser',ROOT/'crates/seata_test']:files.extend(p for p in directory.rglob('*') if p.is_file())
    publication=dict(status='LOCALE_DEPENDENCY_DIFFERENTIAL_PUBLISHED_FULL_GOAL_OPEN',gate=GATE.relative_to(ROOT).as_posix(),catalog_sha256=DIGEST,
        members=596,verification_inputs=497,object_states=dict(IMPLEMENTED=13,MISSING=18),original_case_states=dict(MATCH=57,MISSING=19),
        new_context_records_MATCH=12384,result_fields_per_record=3,property_inputs_not_MATCH=5004,word_input_files_not_MATCH=3,
        rejected_defect_variants=8,unclassified_surviving_variants=1,variant_equivalence_proven=False,package_test_execution_passed=100,
        post_gate_changes='Only current documentation/ledgers and qualification/publication tools; verified production/test bytes unchanged',
        full_core_complete=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,TC_safety_gate_passed=False,
        external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',files=[dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in sorted(set(files))])
    target=family/'publication-20261118.json';need(not target.exists(),'Do not overwrite publication');save(target,publication)
    for row in publication['files']:need(sha(ROOT/row['path'])==row['sha256'],'Publication drift')
    print(json.dumps(dict(publication=target.relative_to(ROOT).as_posix(),sha256=sha(target),files=len(publication['files']),full_core_complete=False)),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('stage',choices=['publish','finalize']);args=parser.parse_args()
    publish() if args.stage=='publish' else finalize()
