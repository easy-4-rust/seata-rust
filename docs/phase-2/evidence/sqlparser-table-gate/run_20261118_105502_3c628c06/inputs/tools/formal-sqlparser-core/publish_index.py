"""Update the current ledger after the immutable IndexMeta gate, preserving history."""
import json
import re
from collections import Counter
from freeze import ROOT, DOC
from qualify import read, sha
from verify import need

GATE = ROOT/'docs/phase-2/evidence/sqlparser-index-gate/run_20261118_042124_511b04b6'
TARGETS = dict(zip([
    'testGetSetValues', 'testIsSetNonUnique', 'testGetSetIndexQualifier', 'testGetSetIndexName',
    'testGetSetType', 'testGetSetAscOrDesc', 'testGetSetCardinality', 'testGetSetOrdinalPosition',
    'testGetSetIndextype', 'testEqualsSameObject', 'testEqualsNullObject', 'testEqualsDifferentType',
    'testEqualsIdenticalObjects', 'testEqualsDifferentObjects', 'testHashCodeConsistency', 'testHashCodeDifference'], [
    'get_set_values', 'is_set_non_unique', 'get_set_index_qualifier', 'get_set_index_name',
    'get_set_type', 'get_set_asc_or_desc', 'get_set_cardinality', 'get_set_ordinal_position',
    'get_set_indextype', 'equals_same_object', 'equals_null_object', 'equals_different_type',
    'equals_identical_objects', 'equals_different_objects', 'hash_code_consistency_original_column_assertions',
    'hash_code_difference_original_column_assertions']))


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', 'utf-8')


def publish():
    digest = sha(GATE/'catalog.json')
    need(digest == '6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774', 'Wrong gate')
    catalog = read(GATE/'catalog.json')['members']
    for row in catalog: need(sha(GATE/row['path']) == row['sha256'], 'Frozen member drift')
    result = read(GATE/'result.json'); inventory = read(DOC/'inventory.json')
    need(inventory['source_cases_MATCH'] == 41, 'Current ledger already advanced')
    for row in inventory['objects']:
        if row['java_type'].endswith('.IndexMeta'): row['state'] = 'IMPLEMENTED'; row.pop('remaining_gap', None)
        if row['state'] == 'IMPLEMENTED':
            row.update(rust_sha256=sha(ROOT/row['rust']), verification_level='V3_GOLDEN_DIFF', evidence='verification/index-result.json')
    for row in inventory['cases']:
        if '.struct.IndexMetaTest#' in row['id']:
            row.update(state='MIRRORED', differential='MATCH', target='crates/seata_test/tests/index_meta.rs#'+TARGETS[row['id'].split('#')[1]])
        if row['state'] == 'MIRRORED': row['evidence'] = 'verification/index-source-diff.json'
    inventory.update(object_states=dict(Counter(row['state'] for row in inventory['objects'])),
        source_cases_MATCH=57, source_cases_MISSING=19, current_gate='../evidence/sqlparser-index-gate/'+GATE.name,
        current_catalog_sha256=digest, module_complete=False,
        Rust_extensions=['JavaString', 'JavaThrowable', 'JavaObject', 'MetadataError', 'InvalidEnumValue', 'ColumnList', 'JavaObjectError'])
    need(inventory['object_states'] == dict(MISSING=18, IMPLEMENTED=13), 'Wrong object census')
    save(DOC/'inventory.json', inventory)
    aliases = []
    sources = [(GATE/'result.json', 'index-result.json'), (GATE/'verification/commands.json', 'index-commands.json'),
        (GATE/'verification/1.99-source-diff.json', 'index-source-diff.json'),
        (GATE/'verification/1.99-column-diff.json', 'index-column-edge-diff.json'),
        (GATE/'verification/1.99-index-diff.json', 'index-edge-diff.json'),
        (GATE/'verification/identity-inputs.json', 'index-identity-inputs.json'),
        (GATE/'oracle/commands.json', 'index-java-commands.json'),
        (GATE.parent/'explicit-acceptance-package-20261118.log', 'index-acceptance-package.log')]
    for source, name in sources:
        target = DOC/'verification'/name; need(not target.exists(), 'Do not overwrite evidence alias')
        target.write_bytes(source.read_bytes())
        aliases.append(dict(path=target.relative_to(DOC).as_posix(), sha256=sha(target), origin=source.relative_to(ROOT).as_posix()))
    save(DOC/'verification/index-evidence-aliases.json', dict(aliases=aliases))
    package_log = (DOC/'verification/index-acceptance-package.log').read_text('utf-8')
    need(sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', package_log))) == 95, 'Explicit package floor differs')
    parity = read(DOC/'source-test-parity.json')
    for row in parity['source_tests']:
        if '/struct/IndexMetaTest.java#' in row['source']:
            row.update(disposition='MIRRORED', result_parity='MATCH', targets=['crates/seata_test/tests/index_meta.rs#'+TARGETS[row['source'].split('#')[1]]])
            for flag in ['contract_preserved','inputs_preserved','assertions_preserved','fixture_state_preserved','cleanup_preserved','case_expansion_complete']:
                row[flag] = True
        if row['disposition'] == 'MIRRORED': row['evidence'] = 'verification/index-source-diff.json'
    parity['acceptance_module'].update(not_run=19, status='PARTIAL', skipped=0,
        artifact='verification/index-acceptance-package.log',
        scoped_test_execution=dict(status='PASS', ordinary_passed=95, failed=0, skipped=0),
        scope='Explicit package95 tests =57 original cases+38 Rust obligations;19 original cases MISSING')
    parity['runs']['java'].update(command='Original SourceJUnitRunner and three peers; exact argv in index-java-commands.json', artifact='verification/index-java-commands.json')
    parity['runs']['rust'].update(command='Exact dual-toolchain24 commands in index-commands.json', artifact='verification/index-commands.json', not_run=19)
    parity['runs']['differential'].update(command='python tools/formal-sqlparser-core/verify_index.py --oracle '+result['oracle'],
        matched=57, not_run=19, artifact='verification/index-source-diff.json')
    parity['generated_Rust_oracle_assets'].append(dict(path='crates/seata_test/tests/fixtures/sqlparser_core/index-edges-java.tsv',
        sha256=sha(ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/index-edges-java.tsv'),
        source=(GATE/'oracle/index-edges-java.tsv').relative_to(ROOT).as_posix(),
        purpose='144 computed observations plus4 same-JVM identity inputs; neither expands original source cases'))
    parity['prior_rust_obligation'] = parity['rust_obligation']
    parity['rust_obligation'] = dict(new_test_count=12, prior_test_count=26,
        targets=['crates/seata_test/tests/index_meta_obligations.rs', 'crates/seata_test/tests/index_java_diff.rs', 'crates/seata_test/tests/object_callback_errors.rs'],
        obligations=['Nullable/shared IndexMeta list, fixed Arrays.asList and mutable ArrayList behavior',
                     '108 list equals and18 live iteration observations from original JDK17',
                     'Original callback failure class/rawUTF16/Arc identity preserved',
                     'Index scalar read order, ignored cardinality and null indextype failure',
                     'Enum identity values are explicit inputs, separate from144 computed results'],
        extra_original_observations=144, prior_original_observations=21, JVM_identity_inputs_not_MATCH=4,
        artifact='verification/index-edge-diff.json')
    parity['Rust_callback_API_change'] = result['Rust_callback_API_change']
    save(DOC/'source-test-parity.json', parity)
    common = [
        f'Java基线：`{inventory["java_pin"]}`；Rust基线：`{inventory["rust_pin"]}`，既有未提交工作保留。',
        '规格事实源：`../wp05-mysql-at-plan.md`、`../source-junit-contract.md`；当前阶段WP05 IMPLEMENTING。',
        '固定完整分母：31对象、12原测试文件、76原案例、0外部源测试资产。',
        'Rust目标：`crates/seata_sqlparser`；验收包`crates/seata_test`，publish=false。',
        '目录retain_segments=1，按用户规则保留struct/util子包；不初始化另一套SDD。',
        '当前对象状态：13 IMPLEMENTED、18 MISSING；扩展对象不计源分子。',
        '当前源案例：57 MATCH、19 MISSING；165补充计算观察另列，4个JVM身份值仅为输入。',
        '57源案例来自23基础值对象、18 ColumnMeta、16 IndexMetaTest；后两项hash测试实际仍针对ColumnMeta。',
        '当前快照：`verification/index-result.json`；全部命令：`verification/index-commands.json`。']
    sections = {
        '迁移路线图.md': ('阶段及剩余工作', [
            '本阶段新增IndexMeta及16原案例，保留nullable values、忽略cardinality的equals和空indextype失败。',
            '下一阶段TableMeta的19原案例；需保留共享ColumnMeta、大小写/Locale、插入顺序和主键集合语义。',
            'SqlServerTableMeta需实现IDENTITY及父对象行为，禁止空代理或STUB计完成。',
            '剩余18对象还含识别器接口、参数容器、TableMetaCache、Defaultable/Sequenceable。',
            'EscapeHandler/ColumnUtils/SPI按原ServiceLoader契约及用户inventory映射继续实施；尚未增加候选依赖。',
            'WP05十四项、WP06、公开业务COMMIT及TC安全门禁均保持未完成。']),
        '对象级对照表.md': ('对象边界', [
            'ColumnMeta和IndexMeta分别独立源文件；Clone保留Java共享句柄身份。',
            'ColumnMeta 608行经内聚审查，仅含一个Java对象与私有状态，允许501–800行例外。',
            'Rust扩展：JavaString、JavaThrowable、JavaObject、MetadataError、InvalidEnumValue、ColumnList、JavaObjectError。',
            'ColumnList实现实际使用的ArrayList与Arrays.asList固定列表，保留null元素、别名和结构变更检查。',
            '原IndexMetaTest最后2项测试操作ColumnMeta，原输入/断言保留，不伪称新增IndexMeta hash源测试。',
            '每个IMPLEMENTED对象均有真实源/目标/证据；布局18缺失继续阻断完整模块。']),
        '语义迁移对照表.md': ('已验证语义与接口变更', [
            'JavaString保留全部UTF16码元；*_java返回无损值，旧UTF8便利视图签名保留。',
            'IndexMeta.equals保持other.values.equals(self.values)方向、同引用短路、忽略cardinality和原空indextype异常。',
            '列表回调前释放锁，比较后按JDK17时点检查modCount；ArrayList.equals保留旧backing，迭代hasNext采用cursor != size。',
            '列表回调后重新读取IndexMeta标量；ColumnMeta原有尾字段重读和不持锁Debug继续保留。',
            'JavaObject三种回调、JavaThrowable文本及from_java_cause现返回Result；调用点已适配，失败不吞为false/0/null。',
            'JavaObjectError保留原异常class、原始UTF16消息及Arc/cause身份；不执行用户Debug回调。',
            'IndexMeta hash忽略values/cardinality；Enum identityHash属JVM运行时输入，显式上下文重放后比较4个计算结果。',
            '108列表equals、18文本迭代、7异常路径及其余边界共144新计算观察；加原21观察为165。']),
        '对象名称一致性检查.md': ('名称、签名及审查', [
            'org.apache.seata.sqlparser最后一级子包映射Rust目录；struct使用r#struct标识。',
            'IndexMeta.java→struct/index_meta.rs；getType/setType→r#type/set_type，与indextype/set_indextype明确区分。',
            'IndexMeta的short/long分别保持i16/i64及极值；nullable采用Option，不隐式归一化。',
            'ColumnMeta.getIsAutoincrement→is_autoincrement_text，isAutoincrement→is_autoincrement。',
            'SQLParsingException保留原类型名；Rust桥新增Result是显式错误传播接口变更。',
            'lib.rs/mod.rs仅声明与重导出；一个源对象一个主文件，原测试名字和完整76身份保留。',
            '静态布局18缺失、0未允许警告；608行ColumnMeta仅为单对象内聚例外。'])}
    for filename, (heading, notes) in sections.items():
        path = DOC/filename; previous = path.read_text('utf-8')
        text = [f'# SQL parser core：{filename[:-3]}', '', '## 当前规格与基线', '']+['- '+x for x in common]
        text += ['', '## '+heading, '']+['- '+x for x in notes]
        text += ['', '## 对象级对照矩阵', '', '| Java对象 | Rust路径 | 状态 |', '|---|---|---|']
        text += ['| '+row['java_type']+' | `'+row['rust']+'` | '+row['state']+' |' for row in inventory['objects']]
        text += ['', '## 当前验证与限制', '', '| 边界 | 实际结果 | 证据 |', '|---|---|---|',
            '| 原JUnit | JDK17全部76通过，13事件反例拒绝 | `verification/index-java-commands.json` |',
            '| 双工具链 | 各434普通测试/89 harness/4doc；143 ignored | `verification/index-commands.json` |',
            '| 原案例 | 57 MATCH，19 MISSING | `verification/index-source-diff.json` |',
            '| 新计算观察 | 144 MATCH；4个身份值为输入 | `verification/index-edge-diff.json` |',
            '| 原补充观察 | 21 MATCH | `verification/index-column-edge-diff.json` |',
            '| 显式验收包 | 95通过=57源案例+38 Rust补充测试 | `verification/index-acceptance-package.log` |',
            '| 错误实现/比较反例 | 9组真实行为失败，8比较反例拒绝 | `verification/index-result.json` |',
            f'| 冻结catalog | {len(catalog)}成员/{result["input_files"]}输入 | `{digest}` |', '',
            '- E0432仅为结构RED；9组隔离mutant均为编译后实际行为失败，未篡改工作区源码。',
            '- 原219 runtime输入、31原对象、12原测试副本及旧组件字节绑定；依赖锁未变。',
            '- 本轮不重跑数据库认证；原AT SQL支持矩阵不扩大；143环境测试忽略照实列出。',
            '- 共享锁延期，外部TC风险未接受；完整core/WP05/WP06/V1及TC安全均未完成。',
            '- 全局源方法还余1078/1135，具体案例还余1270/1327；不能以测试总数冒充源覆盖。', '',
            '<!-- historical-design-appendix-start -->', '## 历史验证及设计附录', '',
            '以下仅为之前的快照和实施注记；旧状态不覆盖上方当前事实。', '', previous]
        path.write_text('\n'.join(text), 'utf-8')
    state = result | dict(gate=GATE.relative_to(ROOT).as_posix(), catalog_sha256=digest,
        current_inventory='docs/phase-2/sqlparser_core/inventory.json', current_source_parity='docs/phase-2/sqlparser_core/source-test-parity.json',
        predecessor='docs/phase-2/wp05-sqlparser-column.json', selected_source_methods_MATCH=57,
        selected_source_methods_MISSING=1078, selected_source_concrete_cases_MATCH=57, selected_source_concrete_cases_MISSING=1270,
        package_test_execution_passed=95, current_specification_state='WP05_IMPLEMENTING_SQLPARSER_CORE_OPEN',
        next_action='Migrate TableMeta19 original cases and remaining18 source objects')
    save(ROOT/'docs/phase-2/wp05-sqlparser-index.json', state)
    (GATE.parent/'README.md').write_text('# 索引元数据与动态异常传播\n\n'
        '状态：本阶段已验证；完整core及全部交付目标继续OPEN，沿用既有WP05/ADR。\n\n'
        '- IndexMeta新增16原案例；当前13 IMPLEMENTED/18 MISSING、57原案例MATCH/19 MISSING。\n'
        '- JDK17原76全部通过；Rust1.99/1.85各434普通测试、89 harness、4doc通过，143 ignored。显式验收包95通过。\n'
        '- 新144计算观察与原21观察一致；4个JVM枚举identity值仅是输入。9个错误实现实际运行失败、8比较及13JUnit反例拒绝。\n'
        '- 列表共享、固定/可变列表、可重入时点和原异常身份已验证。Rust回调扩展API显式改为Result，调用点同步。\n'
        '- 原219运行输入与旧组件保护，依赖锁未变；未重跑数据库。下一步TableMeta及其19原案例。\n'
        '- 共享锁延期，外部TC风险未接受；TC安全/core/WP05/WP06/V1未完成。\n\n'
        f'冻结root `{GATE.name}`；catalog `{digest}`，{len(catalog)}成员/{result["input_files"]}输入。\n\n'
        '[机器状态](../../wp05-sqlparser-index.json) · [当前对象清单](../../sqlparser_core/对象级对照表.md)\n', 'utf-8')
    progress = ROOT/'docs/delivery-progress.md'; previous = progress.read_text('utf-8')
    marker = '## 2026-11-18：索引元数据与动态异常传播'
    need(marker not in previous, 'Progress already published')
    progress.write_text(previous+'\n\n'+marker+'\n\n'
        '- IndexMeta新增16原案例，当前13 IMPLEMENTED/18 MISSING，累计57源MATCH/19待迁移。动态回调Result保留原异常。\n'
        '- 两工具链各434测试、89 harness、4doc；143 ignored。165计算观察MATCH，4身份值只计输入，9行为mutant/8比较反例拒绝。\n'
        f'- [验收证据](phase-2/evidence/sqlparser-index-gate/README.md)与[机器状态](phase-2/wp05-sqlparser-index.json)：{GATE.name}，catalog {digest}。\n'
        '- 下步TableMeta19原案例及剩余18对象；无新DB认证，TC安全和全core/WP05/WP06/V1继续OPEN。\n', 'utf-8')
    readme = ROOT/'README.md'; previous = readme.read_text('utf-8')
    old = 'WP05推进[SQL parser core原对象迁移](docs/phase-2/evidence/sqlparser-column-gate/README.md)：基础值对象、ColumnMeta及无损UTF-16接口已验证，累计41个原案例双工具链差分匹配；完整31对象／76案例迁移仍在实施。'
    need(previous.count(old) == 1, 'README anchor differs')
    readme.write_text(previous.replace(old, 'WP05推进[SQL parser core原对象迁移](docs/phase-2/evidence/sqlparser-index-gate/README.md)：基础值对象、ColumnMeta、IndexMeta及异常传播已验证，累计57个原案例双工具链差分匹配；完整31对象／76案例迁移仍在实施。'), 'utf-8')
    print(json.dumps(dict(gate=GATE.name, source_MATCH=57, source_MISSING=19, IMPLEMENTED=13, MISSING=18)), flush=True)


if __name__ == '__main__': publish()
