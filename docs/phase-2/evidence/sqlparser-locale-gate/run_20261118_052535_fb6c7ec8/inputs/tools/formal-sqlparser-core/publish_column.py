"""Consolidate all31 objects and76 cases after the verified column dependency stage."""
import json
import os
from collections import Counter
from pathlib import Path
from freeze import ROOT, DOC
from qualify import read, sha
from verify import need

GATE = ROOT/'docs/phase-2/evidence/sqlparser-column-gate/run_20261118_030159_4362448c'
COLUMN_TARGETS = {
    'testEqualsAndHashCode': 'equals_and_hash_code', 'testAutoincrement': 'autoincrement',
    'testGetTableCat': 'get_table_cat', 'testSetGetColumnName': 'set_get_column_name',
    'testSetGetDataType': 'set_get_data_type', 'testSetGetDataTypeName': 'set_get_data_type_name',
    'testSetGetColumnSize': 'set_get_column_size', 'testSetGetDecimalDigits': 'set_get_decimal_digits',
    'testSetGetNumPrecRadix': 'set_get_num_prec_radix', 'testSetGetNullAble': 'set_get_null_able',
    'testSetGetRemarks': 'set_get_remarks', 'testSetGetColumnDef': 'set_get_column_def',
    'testSetGetSqlDataType': 'set_get_sql_data_type', 'testSetGetSqlDatetimeSub': 'set_get_sql_datetime_sub',
    'testSetGetCharOctetLength': 'set_get_char_octet_length', 'testSetGetOrdinalPosition': 'set_get_ordinal_position',
    'testSetGetIsOnUpdate': 'set_get_on_update', 'testSetGetIsCaseSensitive': 'set_get_case_sensitive'}


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', 'utf-8')


def publish():
    need(sha(GATE/'catalog.json') == '1e54b372ce3454028d650bb7d46e72f25a3a4d42c724441d1ec04756ace213da', 'Wrong exact gate')
    for member in read(GATE/'catalog.json')['members']:
        need(sha(GATE/member['path']) == member['sha256'], 'Frozen gate drift')
    result = read(GATE/'result.json')
    manifest = read(DOC/'inventory.json')
    need(manifest['source_cases_MATCH'] == 23, 'Do not overwrite a later current ledger')
    expected = {row['id'] for row in read(GATE/'verification/1.99-source-diff.json')}
    for row in manifest['objects']:
        if row['java_type'].endswith(('.ColumnMeta', '.SQLParsingException', '.SqlSequenceExpr')):
            row['state'] = 'IMPLEMENTED'
            row.pop('remaining_gap', None)
            row.update(rust_sha256=sha(ROOT/row['rust']), verification_level='V3_GOLDEN_DIFF', evidence='verification/column-result.json')
    for row in manifest['cases']:
        if '.struct.ColumnMetaTest#' in row['id']:
            method = row['id'].split('#')[1]
            row.update(state='MIRRORED', differential='MATCH',
                       target='crates/seata_test/tests/column_meta.rs#'+COLUMN_TARGETS[method])
        if row['id'] in expected:
            row['evidence'] = 'verification/column-source-diff.json'
    manifest.update(object_states=dict(Counter(row['state'] for row in manifest['objects'])),
                    source_cases_MATCH=41, source_cases_MISSING=35, current_gate=os.path.relpath(GATE, DOC).replace('\\','/'),
                    current_catalog_sha256=sha(GATE/'catalog.json'), module_complete=False,
                    Rust_extensions=['JavaString', 'JavaThrowable', 'JavaObject', 'MetadataError', 'InvalidEnumValue'])
    need(manifest['object_states'] == {'MISSING':19, 'IMPLEMENTED':12}, 'Object census differs')
    save(DOC/'inventory.json', manifest)
    aliases = []
    for source, name in [(GATE/'result.json', 'column-result.json'),
        (GATE/'verification/1.99-source-diff.json', 'column-source-diff.json'),
        (GATE/'verification/1.99-edge-diff.json', 'column-edge-diff.json'),
        (GATE/'verification/commands.json', 'column-commands.json'),
        (GATE/'oracle/commands.json', 'column-java-commands.json')]:
        target = DOC/'verification'/name
        need(not target.exists(), 'Do not overwrite earlier evidence alias')
        target.write_bytes(source.read_bytes())
        aliases.append(dict(path=target.relative_to(DOC).as_posix(), sha256=sha(target), immutable_origin=str(source.relative_to(ROOT))))
    save(DOC/'verification/column-evidence-aliases.json', dict(aliases=aliases))
    parity = read(DOC/'source-test-parity.json')
    for row in parity['source_tests']:
        method = row['source'].split('#')[1]
        if '/struct/ColumnMetaTest.java#' in row['source']:
            row.update(disposition='MIRRORED', result_parity='MATCH',
                       targets=['crates/seata_test/tests/column_meta.rs#'+COLUMN_TARGETS[method]])
            for flag in ['contract_preserved','inputs_preserved','assertions_preserved','fixture_state_preserved','cleanup_preserved','case_expansion_complete']:
                row[flag] = True
        if row['disposition'] == 'MIRRORED':
            row['evidence'] = 'verification/column-source-diff.json'
    parity['acceptance_module'].update(not_run=35, status='PARTIAL',
        command='cargo test --offline --locked --workspace --all-features --all-targets',
        artifact='verification/column-commands.json',
        scope='41 of76 source cases; whole module migration remains incomplete',
        scoped_test_execution=dict(status='PASS', ordinary_passed=406, ignored=143, doc=4, scope='whole Rust workspace'))
    parity['runs']['java'].update(artifact='verification/column-java-commands.json')
    parity['runs']['rust'].update(artifact='verification/column-commands.json', not_run=35)
    parity['runs']['differential'].update(artifact='verification/column-source-diff.json', matched=41, not_run=35,
        command='python tools/formal-sqlparser-core/verify_column.py --oracle '+result['oracle'])
    parity['generated_Rust_oracle_assets'].append(dict(path='crates/seata_test/tests/fixtures/sqlparser_core/column-edges-java-v2.tsv',
        sha256=sha(ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/column-edges-java-v2.tsv'),
        source=str((GATE/'oracle/edges-java.tsv').relative_to(ROOT)), purpose='21 additional observations; not source JUnit cases'))
    parity['rust_obligation'] = dict(new_test_count=19, prior_test_count=7,
        targets=['crates/seata_test/tests/core_utf16.rs', 'crates/seata_test/tests/column_meta_obligations.rs',
                 'crates/seata_test/tests/column_java_diff.rs'],
        obligations=['All65536 UTF16 code units retain raw values', 'Java String.hashCode overflow',
                     'Raw message/cause text and cause identity', 'Shared column identity and null resets',
                     'Dynamic Object preserves boxed types', 'User Drop and Debug cannot deadlock on held metadata lock',
                     'equals/hash/toString callback read order', '196608 YES ignore-case candidates match original JDK17'],
        extra_original_observations=21, artifact='verification/column-edge-diff.json')
    save(DOC/'source-test-parity.json', parity)
    common = [f'- Java基线：`{manifest["java_pin"]}`。', f'- Rust基线：`{manifest["rust_pin"]}`；未提交的既有工作保留。',
        '- 审计日期：2026-11-18；文档状态IMPLEMENTING，完整模块未完成。',
        '- 规格事实源：`../wp05-mysql-at-plan.md`、`../source-junit-contract.md`。',
        '- 固定完整分母：31对象、12原测试文件、76原案例、0外部源测试资产。',
        '- Rust目标：`crates/seata_sqlparser/src`；测试包`crates/seata_test`，publish=false。',
        '- 目录规则：retain_segments=1，用户规则覆盖技能默认2；保留struct/util子包。',
        '- 当前对象状态：12 IMPLEMENTED、19 MISSING；不以扩展对象或测试数量增加源分子。',
        '- 当前源案例：41 MATCH、35 MISSING；21补充观察另列，不算入76源分母。',
        '- 当前代码快照与全部命令：`verification/column-result.json`、`verification/column-commands.json`。']
    sections = {
        '迁移路线图.md': ('依赖阶段与剩余工作', [
            '首个值对象依赖阶段已完成23案例；本阶段新增ColumnMeta的18原案例。',
            '无损UTF16桥消除了SQLParsingException与SqlSequenceExpr旧PARTIAL输入域缺口。',
            '继续IndexMeta：16个原案例，保留nullable values、忽略cardinality的equals和原空indextype失败。',
            '继续TableMeta：19个原案例，必须共享同一ColumnMeta，保持忽略大小写、顺序、主键集合及错误。',
            'SqlServerTableMeta需保留IDENTITY标志及TableMeta行为，不创建空代理充数。',
            '之后完成识别器trait、参数容器、TableMetaCache、Defaultable/Sequenceable。',
            'EscapeHandler/ColumnUtils工厂链继续按原ServiceLoader与用户inventory映射实施。',
            '全模块统一门禁仍阻断；不得以这41案例替代WP05十四项和WP06验收。']),
        '对象级对照表.md': ('对象状态与边界', [
            'ColumnMeta文件实现完整20字段、访问器、equals/hashCode/toString和共享引用。',
            '606行经独立内聚审查：单个源对象及其私有状态，允许501–800行例外；不拆伪对象。',
            'JavaString、JavaThrowable、JavaObject、MetadataError、InvalidEnumValue均为Rust扩展。',
            'JavaObject让调用方适配实际动态对象；内置Integer/Long等保持不同类型，不按文本合并。',
            'Clone为同一ColumnMeta句柄别名，不会把Java共享对象复制为独立值。',
            'source-test-parity保留全部76身份，宏生成的访问器测试仍由实际Rust测试执行观察绑定。',
            '每个IMPLEMENTED对象均有真实确定路径及source/target/evidence；文件存在不单独计完成。',
            '当前矩阵不包含Rust扩展及原测试夹具；历史9/2/20仅保留在明确历史附录。']),
        '语义迁移对照表.md': ('已实现的原语义', [
            'JavaString以Arc<[u16]>保留全部码元，包括NUL和孤立代理码元；UTF8仅作可选缓存。',
            '原getSequence/getFunction/getMessage映射*_java无损入口，旧UTF8便利接口签名保留。',
            'JavaThrowable允许toString返回null，原因Arc身份保留；Java完整toString以JavaString返回。',
            'ColumnMeta的nullable字符串不归一化；primitive初值0/false，动态字段初值null。',
            'equals保持other字段先调用的方向；同一句柄立即true、null/其他类型false。',
            'hashCode保持20字段原顺序、Boolean 1231/1237及有符号i32回绕；不使用Rust随机Hash代替。',
            'toString按原字段顺序保留引号、null字面量及原UTF16；Rust Display非法码元仅用于显式诊断转义。',
            '动态Object回调前释放锁，回调后重新读取尾字段，保留原单线程可重入副作用。',
            '替换动态字段在锁外释放旧值；Debug只输出句柄身份，避免用户回调锁死。',
            'YES忽略大小写保留U+017F长s；196608个候选按原JDK17逐项核验，禁止仅ASCII比较。']),
        '对象名称一致性检查.md': ('名称、签名与路径映射', [
            '固定Java包根org.apache.seata.sqlparser；同名Java子包映射最后一级目录。',
            '磁盘目录struct保持原名，Rust模块标识使用r#struct；不改成错误的structures目录。',
            'ColumnMeta.java→struct/column_meta.rs；一个Java对象一个真实主文件。',
            '原getIsAutoincrement→is_autoincrement_text，原isAutoincrement→is_autoincrement，避免类型歧义。',
            '原getIsNullAble→is_null_able_text，保留与整数null_able的区别。',
            '原setOnUpdate(onUpdate)→set_on_update(on_update)，参数语义不变。',
            '原protected表/模式名getter在Rust中提供公开只读访问，代替Java继承可见性；不改变字段值。',
            'Rust读取/写入以Result报告锁中毒，不能伪造空字段或成功。',
            'SQLParsingException保留原类型名；NotPlaceHolderExprTest原测试拼写不更改。',
            '静态布局19缺失明确阻断；已审查606行例外，新增代码无未允许警告。'])}
    for filename, (heading, notes) in sections.items():
        path = DOC/filename
        previous = path.read_text('utf-8')
        current = [f'# SQL parser core：{filename[:-3]}', '', '## 当前规格与基线', '']+common
        current += ['', '## '+heading, '']+['- '+note for note in notes]
        current += ['', '## 对象级对照矩阵', '', '| Java对象 | Rust路径 | 状态 |', '|---|---|---|']
        current += ['| '+row['java_type']+' | `'+row['rust']+'` | '+row['state']+' |' for row in manifest['objects']]
        current += ['', '## 当前验收证据与严格门禁', '', '| 边界 | 实际结果 | 证据 |', '|---|---|---|',
            '| 原JUnit | 固定JDK17原76通过，13事件反例拒绝 | `verification/column-java-commands.json` |',
            '| 双工具链 | Rust1.99/1.85各406通过、85 harness、4doc；143 ignored | `verification/column-commands.json` |',
            '| 原案例 | 累计41 MATCH，余35 MISSING | `verification/column-source-diff.json` |',
            '| 补充观察 | 21 MATCH；5组错误实现共8运行时失败、8比较反例拒绝 | `verification/column-edge-diff.json` |',
            '| 旧组件保护 | 原组件字节和依赖锁未变；本轮不重跑DB认证 | `verification/column-result.json` |',
            '| 当前catalog | 483成员、422输入 | `1e54b372ce3454028d650bb7d46e72f25a3a4d42c724441d1ec04756ace213da` |', '',
            '- MISSING、PARTIAL、UNVERIFIED均阻止完整模块关闭；原76分母保持不变。',
            '- 新增18原案例与19个Rust补充测试分列；整个测试包67测试不冒充67源案例。',
            '- 初始E0432仅为结构RED；Unicode比较及4项可重入失败是实际行为RED，已修复复核。',
            '- 本轮只计最终verification_20261118_025807_344089，早期锚点失败和文档前候选保留诊断。',
            '- 共享锁延期、TC安全未通过、外部TC风险未接受；完整core/WP05/WP06/V1均未完成。',
            '- 下一步继续IndexMeta/TableMeta及35原案例，不创建或切换分支、安装工具、初始化SDD。', '',
            '<!-- historical-design-appendix-start -->', '## 历史设计附录', '',
            '以下为本阶段之前的已验证快照及实施注记。其9/2/20、23/53和旧PARTIAL状态仅解释历史，不覆盖上方当前事实。', '', previous]
        path.write_text('\n'.join(current), 'utf-8')
    state = result | dict(gate=str(GATE.relative_to(ROOT)), catalog_sha256=sha(GATE/'catalog.json'),
        current_inventory='docs/phase-2/sqlparser_core/inventory.json', current_source_parity='docs/phase-2/sqlparser_core/source-test-parity.json',
        predecessor='docs/phase-2/wp05-sqlparser-core-values.json', selected_source_methods_MATCH=41,
        selected_source_methods_MISSING=1094, selected_source_concrete_cases_MATCH=41, selected_source_concrete_cases_MISSING=1286,
        resolved_gaps=['UTF16 String lossless domain for sequence/exception', 'Unicode YES case equivalence',
                       'Debug lock reentrancy', 'Object callback tail-read order'],
        current_specification_state='WP05_IMPLEMENTING_SQLPARSER_CORE_OPEN', next_action='Migrate IndexMeta and TableMeta remaining35 source cases')
    save(ROOT/'docs/phase-2/wp05-sqlparser-column.json', state)
    (GATE.parent/'README.md').write_text('# 列元数据与无损UTF-16依赖阶段\n\n'
        '状态：本阶段已验证，完整core迁移仍OPEN；唯一规格事实源仍为既有WP05/ADR。\n\n'
        '- 新增ColumnMeta完整20字段及共享身份，修复两个旧对象的UTF16缺口。当前12 IMPLEMENTED／19 MISSING。\n'
        '- 固定原Java76测试新跑通过；累计41原案例双Rust差分MATCH，剩35案例待迁移。另21补充观察独立计数。\n'
        '- Rust1.99/1.85各406普通测试、85 harness、4doc通过，143 ignored。5组错误实现共8行为失败、8比较及13JUnit反例拒绝。\n'
        '- Debug持锁回调与equals/hash/toString快照顺序两项P2有真实RED并修复；源码独立复核通过。\n'
        '- 606行ColumnMeta仅对应一个Java对象及私有状态，内聚例外经审查；19对象缺失继续阻断完整模块。\n'
        '- 无数据库现场重跑，无依赖锁升级；共享锁延期，TC/WP05/WP06/V1未完成。\n\n'
        f'最终root `{GATE.name}`；catalog `{sha(GATE/"catalog.json")}`，483成员／422输入。\n\n'
        '[机器状态](../../wp05-sqlparser-column.json) · [当前迁移清单](../../sqlparser_core/对象级对照表.md)\n', 'utf-8')
    progress = ROOT/'docs/delivery-progress.md'
    marker = '## 2026-11-18：列元数据与无损UTF-16依赖阶段'
    previous = progress.read_text('utf-8'); need(marker not in previous, 'Progress already exists')
    progress.write_text(previous+'\n\n'+marker+'\n\n'
        '- 新增ColumnMeta及全部18源案例，消除两个旧对象UTF16 PARTIAL缺口；当前12 IMPLEMENTED／19 MISSING，累计41源MATCH／35待迁移。\n'
        '- 两Rust各406普通／85 harness／143 ignored／4doc；21Java补充观察MATCH，5变异共8行为失败，8比较／13JUnit反例拒绝。两项重入P2与ASCII长s遗漏均有真实RED并修复。\n'
        '- [阶段验收](phase-2/evidence/sqlparser-column-gate/README.md)与[机器状态](phase-2/wp05-sqlparser-column.json)：root '+GATE.name+'，catalog '+sha(GATE/'catalog.json')+'，483成员／422输入。\n'
        '- 完整core、选定源模块1094方法／1286案例、公开业务COMMIT、TC安全、WP05/WP06/V1继续OPEN；共享锁延期，外部TC风险未接受。\n', 'utf-8')
    print(json.dumps(dict(current_states=manifest['object_states'], source_MATCH=41, remaining_cases=35, full_core_complete=False)), flush=True)


if __name__ == '__main__':
    publish()
