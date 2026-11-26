"""Publish the shared TableMeta set prerequisite without increasing source completion counts."""
import json
import re
from freeze import ROOT, DOC
from qualify import read, sha
from verify import need

GATE=ROOT/'docs/phase-2/evidence/sqlparser-table-prerequisites-gate/run_20261118_044535_5f39165d'
DIGEST='9df62e28460c66b2aae94071d417f2f10ce3c2a46eedf02af237b3daeb6845bb'


def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n','utf-8')


def publish():
    need(sha(GATE/'catalog.json')==DIGEST,'Wrong gate')
    for row in read(GATE/'catalog.json')['members']:need(sha(GATE/row['path'])==row['sha256'],'Gate member drift')
    result=read(GATE/'result.json');inventory=read(DOC/'inventory.json')
    need(inventory['source_cases_MATCH']==57 and inventory['source_cases_MISSING']==19 and
         inventory['current_catalog_sha256']=='6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774','Current ledger advanced')
    aliases=[]
    for source,name in [(GATE/'result.json','table-set-result.json'),(GATE/'verification/commands.json','table-set-commands.json'),
        (GATE/'verification/1.99-source-diff.json','table-set-source-diff.json'),
        (GATE/'verification/1.99-table-set-diff.json','table-set-diff.json'),
        (GATE/'oracle/commands.json','table-set-java-commands.json'),
        (GATE.parent/'explicit-acceptance-package-20261118.log','table-set-acceptance-package.log')]:
        target=DOC/'verification'/name;need(not target.exists(),'Do not overwrite alias');target.write_bytes(source.read_bytes())
        aliases.append(dict(path=target.relative_to(DOC).as_posix(),sha256=sha(target),origin=source.relative_to(ROOT).as_posix()))
    save(DOC/'verification/table-set-evidence-aliases.json',dict(aliases=aliases))
    need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',(DOC/'verification/table-set-acceptance-package.log').read_text('utf-8'))))==98,'Explicit acceptance package floor differs')
    inventory.update(current_gate='../evidence/sqlparser-table-prerequisites-gate/'+GATE.name,current_catalog_sha256=DIGEST)
    need('CaseInsensitivePkSet' not in inventory['Rust_extensions'],'Extension already published')
    inventory['Rust_extensions'].append('CaseInsensitivePkSet')
    inventory['compatibility_inputs']=[dict(path='crates/seata_sqlparser/src/data/java17_character_case.bin',
        sha256=sha(ROOT/'crates/seata_sqlparser/src/data/java17_character_case.bin'),records=2799,
        role='INPUT_NOT_RESULT',source='JDK17.0.12 Character simple upper/lower; not Locale String casing')]
    for row in inventory['objects']:
        if row['state']=='IMPLEMENTED':
            need(sha(ROOT/row['rust'])==row['rust_sha256'],'Original object source unexpectedly changed')
            row['evidence']='verification/table-set-result.json'
    for row in inventory['cases']:
        if row['state']=='MIRRORED':row['evidence']='verification/table-set-source-diff.json'
    save(DOC/'inventory.json',inventory)
    parity=read(DOC/'source-test-parity.json')
    for row in parity['source_tests']:
        if row['disposition']=='MIRRORED':row['evidence']='verification/table-set-source-diff.json'
    parity['acceptance_module'].update(artifact='verification/table-set-acceptance-package.log',not_run=19,status='PARTIAL',
        scoped_test_execution=dict(status='PASS',ordinary_passed=98,failed=0,skipped=0),
        scope='98 package tests=57 original cases+41 Rust obligations; TableMeta19 cases and18 source objects remain MISSING')
    parity['runs']['rust'].update(artifact='verification/table-set-commands.json',command='22 actual dual-toolchain and runtime-mutant commands; partial source floor unchanged')
    parity['runs']['differential'].update(artifact='verification/table-set-source-diff.json',
        command='python tools/formal-sqlparser-core/verify_table_set.py --oracle '+result['oracle'])
    # Java source JUnit receipt remains the inherited76 run; do not relabel a JDK-only peer as a fresh JUnit run.
    parity['inherited_original_JUnit_76_not_rerun']=True
    for target,origin,count in [('java17-comparisons.tsv','java17-comparisons.tsv',22492),('table-primary-sets-v2.tsv','table-primary-sets.tsv',878)]:
        path='crates/seata_test/tests/fixtures/sqlparser_core/'+target
        parity['generated_Rust_oracle_assets'].append(dict(path=path,sha256=sha(ROOT/path),source=(GATE/'oracle'/origin).relative_to(ROOT).as_posix(),
            computed_observations=count,purpose='Supplementary computed JDK/original TableMeta observations; not source JUnit cases'))
    parity['dependency_obligation_history']=[parity.pop('prior_rust_obligation'),parity['rust_obligation']]
    parity['rust_obligation']=dict(new_test_count=3,prior_test_count=38,
        targets=['crates/seata_test/tests/table_primary_set.rs'],extra_original_observations=23370,
        compatibility_input_records_not_MATCH=2799,artifact='verification/table-set-diff.json',
        obligations=['Original JDK17 compareToIgnoreCase including raw UTF16 and supplementary lookbehind',
            'Original nontransitive comparator tree insertion/color/rotation, iteration and root-path membership',
            'First spelling, immutable collection failure, null lookup, native set equals/hash quirks'])
    save(DOC/'source-test-parity.json',parity)
    common_notes=[
        '本轮实现TableMeta共享依赖CaseInsensitivePkSet及固定JDK17比较器；TableMeta本体仍为MISSING，不增加原对象或案例分子。',
        '新23,370计算观察=22,492比较+839组原TableMeta集合+36集合equals+3其他观察，全部双工具链MATCH。',
        '2,799条简单字符映射为生产兼容性INPUT，不计MATCH或原测试资产；4条既有JVM枚举身份输入也不计MATCH。',
        'JDK比较器可能形成B<C<D<B：B=d800dc00，C=d800d800dc00，D=e000；原[B,C,D]插入输出[D,B,C]。',
        '已用原Java产生真实RED，修复为原红黑树插入、变色、旋转、根路径查询和中序迭代；不使用排序数组替代。',
        '不可变集合保留首拼写、空/非空查null差异、空addAll仍抛UOE，以及equals与原拼写hash的不一致。',
        '实际JDK的java/javac/modules/release/src.zip五文件执行前后绑定，另保留219条原运行输入。原JUnit76继承上一验收，未声称本轮新跑。',
        'String默认Locale大小写转换和多码点展开属于下一步独立契约；本比较器不证明LowerCaseLinkHashMap或containsPK已实现。',
        '完整31对象/76案例仍为13 IMPLEMENTED、18 MISSING和57 MATCH、19 MISSING；WP05十四项、WP06、V1及TC安全保持OPEN。']
    for path in DOC.glob('*.md'):
        current,marker,history=path.read_text('utf-8').partition('<!-- historical-design-appendix-start -->')
        current=current.replace('各434普通测试/89 harness/4doc','各437普通测试/90 harness/4doc')
        current=current.replace('95通过=57源案例+38 Rust补充测试','98通过=57源案例+41 Rust补充测试')
        current=current.replace('9组真实行为失败，8比较反例拒绝','本依赖7组真实行为失败，8比较反例拒绝')
        current=current.replace('verification/index-result.json','verification/table-set-result.json').replace('verification/index-commands.json','verification/table-set-commands.json')
        current=current.replace('verification/index-source-diff.json','verification/table-set-source-diff.json').replace('verification/index-acceptance-package.log','verification/table-set-acceptance-package.log')
        current=current.replace('534成员/444输入','537成员/468输入').replace('6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774',DIGEST)
        current=current.replace('9组隔离mutant均为编译后实际行为失败','本依赖7组隔离mutant均为编译后实际行为失败')
        current+='\n## TableMeta主键集合依赖（已验证）\n\n'+'\n'.join('- '+note for note in common_notes)+'\n\n'
        path.write_text(current+marker+history,'utf-8')
    state=result|dict(gate=GATE.relative_to(ROOT).as_posix(),catalog_sha256=DIGEST,
        current_inventory='docs/phase-2/sqlparser_core/inventory.json',current_source_parity='docs/phase-2/sqlparser_core/source-test-parity.json',
        predecessor='docs/phase-2/wp05-sqlparser-index.json',package_test_execution_passed=98,
        selected_source_methods_MATCH=57,selected_source_methods_MISSING=1078,
        selected_source_concrete_cases_MATCH=57,selected_source_concrete_cases_MISSING=1270,
        current_specification_state='WP05_IMPLEMENTING_SQLPARSER_CORE_OPEN')
    save(ROOT/'docs/phase-2/wp05-sqlparser-table-prerequisites.json',state)
    (GATE.parent/'README.md').write_text('# TableMeta主键集合依赖\n\n'
        '沿用既有WP05/ADR及完整31对象/76案例规格。此依赖阶段已验证，TableMeta本体仍待实现。\n\n'
        '- 固定JDK17比较器及不可变主键树已实现；发现并修复非传递比较器下排序数组改变原树行为的P2，有真实RED。\n'
        '- 新23,370计算观察MATCH；2,799字符映射仅为输入。原57源案例及165补充观察回归MATCH，原19案例/18对象仍缺失。\n'
        '- Rust1.99/1.85各437普通测试、90 harness、4doc通过，143 ignored；显式验收包98通过。\n'
        '- 7错误实现真实运行失败，8比较反例拒绝；219原运行输入和5个实际JDK文件绑定。原JUnit76为继承证据。\n'
        '- 无新增依赖或DB现场认证；共享锁延期，外部TC风险未接受，完整core/WP05/WP06/V1及TC安全未完成。\n\n'
        f'root `{GATE.name}`；catalog `{DIGEST}`，537成员/468输入。\n\n'
        '[机器状态](../../wp05-sqlparser-table-prerequisites.json) · [对象清单](../../sqlparser_core/对象级对照表.md)\n','utf-8')
    progress=ROOT/'docs/delivery-progress.md';old=progress.read_text('utf-8');marker='## 2026-11-18：TableMeta主键集合依赖'
    need(marker not in old,'Progress already published')
    progress.write_text(old+'\n\n'+marker+'\n\n'
        '- 固定JDK17比较器及主键红黑树实现，原非传递比较器导致排序Vec偏差的P2有实际RED并修复。\n'
        '- 23,370新计算观察MATCH，2,799映射只计输入；两工具链各437测试/90 harness/4doc，143 ignored，7运行时mutant/8比较反例拒绝。\n'
        '- [证据](phase-2/evidence/sqlparser-table-prerequisites-gate/README.md)；[机器状态](phase-2/wp05-sqlparser-table-prerequisites.json)。\n'
        '- 未将依赖实现算作TableMeta完成：源对象仍13/31、源案例仍57/76；继续默认Locale映射、TableMeta19原案例及剩余18对象。\n','utf-8')
    readme=ROOT/'README.md';old=readme.read_text('utf-8')
    anchor='完整31对象／76案例迁移仍在实施。'
    need(old.count(anchor)==1,'README anchor differs')
    readme.write_text(old.replace(anchor,anchor+' [TableMeta主键集合依赖](docs/phase-2/evidence/sqlparser-table-prerequisites-gate/README.md)已验证，主对象仍待完成。'),'utf-8')
    print(json.dumps(dict(source_MATCH=57,source_MISSING=19,new_computed_observations_MATCH=23370,full_core_complete=False)),flush=True)


if __name__=='__main__':publish()
