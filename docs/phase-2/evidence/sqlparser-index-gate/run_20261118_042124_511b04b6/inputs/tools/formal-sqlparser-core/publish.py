"""Consolidate current module ledgers after the frozen successful dependency stage."""
import argparse
import hashlib
import json
import os
from collections import Counter
from pathlib import Path
from freeze import ROOT, DOC, FIRST
from qualify import read, sha
from verify import need

MIRRORS = {
    'EscapeSymbolTest#testGetLeftSymbol': 'escape_left_symbol',
    'EscapeSymbolTest#testGetRightSymbol': 'escape_right_symbol',
    'SQLTypeTest#testValue': 'sql_type_value', 'SQLTypeTest#testValueOf': 'sql_type_value_of',
    'SQLTypeTest#testValueOfInvalid': 'sql_type_invalid',
    'SQLParsingExceptionTest#testConstructorWithMessage': 'exception_message',
    'SQLParsingExceptionTest#testConstructorWithMessageAndCause': 'exception_message_and_cause',
    'SQLParsingExceptionTest#testConstructorWithCause': 'exception_cause',
    'struct.IndexTypeTest#testValue': 'index_type_value',
    'struct.IndexTypeTest#testValueOf': 'index_type_value_of',
    'struct.IndexTypeTest#testInvalidValueOf': 'index_type_invalid',
    'struct.SqlSequenceExprTest#testDefaultConstructor': 'sequence_default',
    'struct.SqlSequenceExprTest#testParameterizedConstructor': 'sequence_constructor',
    'struct.SqlSequenceExprTest#testSetGetSequence': 'sequence_set_sequence',
    'struct.SqlSequenceExprTest#testSetGetFunction': 'sequence_set_function',
    'struct.SqlMethodExprTest#testGet': 'method_get',
    'struct.SqlMethodExprTest#testToString': 'method_string',
    'struct.SqlDefaultExprTest#testGet': 'default_get',
    'struct.SqlDefaultExprTest#testToString': 'default_string',
    'struct.NullTest#testGet': 'null_get', 'struct.NullTest#testToString': 'null_string',
    'struct.NotPlaceHolderExprTest#testGet': 'not_placeholder_get',
    'struct.NotPlaceHolderExprTest#testToString': 'not_placeholder_string',
}


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', 'utf-8')


def publish(gate):
    gate = gate.resolve()
    result = read(gate/'result.json')
    need(sha(gate/'catalog.json') == 'b0056e0927ef6a565c07f7c39ee49b2cc7fed58ad77bc08bf47bec7acec989cd', 'Wrong selected stage gate')
    for member in read(gate/'catalog.json')['members']:
        need(sha(gate/member['path']) == member['sha256'], 'Frozen stage member changed')
    inventory = read(DOC/'inventory.json')
    need(all(row['state'] == 'MISSING' for row in inventory['objects']), 'Do not overwrite an already audited ledger')
    anchor = Path(os.path.relpath(gate, DOC)).as_posix()
    matches = {row['id'] for row in read(gate/'verification/1.99-differential.json')}
    need(matches == {'org.apache.seata.sqlparser.'+key for key in MIRRORS}, 'Wrong mirror mapping')
    for row in inventory['objects']:
        name = row['java_type'].split('.')[-1]
        if name in FIRST:
            row['state'] = 'PARTIAL' if name in {'SQLParsingException', 'SqlSequenceExpr'} else 'IMPLEMENTED'
            row['evidence'] = anchor+'/result.json'
            row['rust_sha256'] = sha(ROOT/row['rust'])
            row['verification_level'] = 'V3_GOLDEN_DIFF'
            if row['state'] == 'PARTIAL':
                row['remaining_gap'] = 'Rust UTF-8 String cannot represent Java String containing lone UTF-16 surrogate code units'
    parity = []
    for row in inventory['cases']:
        short = row['id'].removeprefix('org.apache.seata.sqlparser.')
        matched = short in MIRRORS
        source = row['source'].removeprefix('sqlparser/seata-sqlparser-core/')+'#'+row['id'].split('#')[1]
        item = dict(source=source, case_id='default',
                    targets=['crates/seata_test/tests/core_values.rs#'+MIRRORS[short]] if matched else [],
                    disposition='MIRRORED' if matched else 'MISSING', result_parity='MATCH' if matched else 'NOT_RUN')
        for flag in ['contract_preserved', 'inputs_preserved', 'assertions_preserved',
                     'fixture_state_preserved', 'cleanup_preserved', 'case_expansion_complete']:
            item[flag] = matched
        if matched:
            item['evidence'] = anchor+'/verification/1.99-differential.json'
            row.update(state='MIRRORED', differential='MATCH', target=item['targets'][0], evidence=item['evidence'])
        parity.append(item)
    inventory.update(object_states=dict(Counter(row['state'] for row in inventory['objects'])),
                     source_cases_MATCH=23, source_cases_MISSING=53, current_gate=anchor,
                     current_catalog_sha256=sha(gate/'catalog.json'), module_complete=False)
    save(DOC/'inventory.json', inventory)
    source_parity = dict(schema=1, java_baseline=inventory['java_pin'], rust_baseline=inventory['rust_pin'],
        acceptance_module=dict(package='seata-test', manifest='crates/seata_test/Cargo.toml', publish=False,
            components=['seata-sqlparser'], command='cargo test --offline --locked --workspace --all-features --all-targets',
            status='PARTIAL', failed=0, skipped=143, not_run=53, artifact=anchor+'/result.json'),
        source_tests=parity, assets=[], original_source_assets=0,
        copied_original_test_sources=12,
        generated_Rust_oracle_assets=[dict(path='crates/seata_test/tests/fixtures/sqlparser_core/java-constants.tsv',
                                          sha256=sha(ROOT/'crates/seata_test/tests/fixtures/sqlparser_core/java-constants.tsv'),
                                          source=anchor+'/oracle/java-constants.tsv')],
        runs=dict(java=dict(command='CoreValueOracle and SourceJUnitRunner; exact argv in frozen oracle/commands.json',
                            status='PASS', failed=0, skipped=0, not_run=0, artifact=anchor+'/oracle/commands.json'),
                  rust=dict(command='exact dual-toolchain commands in frozen verification/rust-commands.json',
                            status='PARTIAL', failed=0, skipped=143, not_run=53, artifact=anchor+'/verification/rust-commands.json'),
                  differential=dict(command='tools/formal-sqlparser-core/verify.py --oracle '+result['source_oracle'],
                            status='PARTIAL', matched=23, mismatched=0, harness_failures=0, not_run=53,
                            artifact=anchor+'/verification/1.99-differential.json')))
    save(DOC/'source-test-parity.json', source_parity)
    for path in DOC.glob('*.md'):
        text = path.read_text('utf-8')
        text = text.replace('当前快照：31 MISSING；76 NOT_RUN；不继承 Java 通过为 Rust 通过。',
                            '当前快照：9 IMPLEMENTED、2 PARTIAL、20 MISSING；23 MATCH、53 NOT_RUN。')
        text = text.replace('第一依赖阶段实现11个基础值对象及23个原案例。',
                            '第一依赖阶段已新增11个真实基础对象；9 IMPLEMENTED、2 PARTIAL；23原案例差分MATCH。')
        for row in inventory['objects']:
            prefix = '| '+row['java_type']+' | `'+row['rust']+'` | '+row['dependency_stage']+' | '
            text = text.replace(prefix+'MISSING |', prefix+row['state']+' |')
        text += '\n## 本轮实际验证证据\n\n'
        text += '| 边界 | 结果 | 证据 |\n|---|---|---|\n'
        text += '| 原Java JUnit | 76通过，0禁用，13反例拒绝 | `'+anchor+'/oracle/junit-validation.json` |\n'
        text += '| 基础源案例 | 双Rust各23 MATCH，余53 MISSING | `'+anchor+'/verification/1.99-differential.json` |\n'
        text += '| 完整常量 | 48 SQLType＋4 IndexType＋34 JDBC＋2解析器，共88匹配 | `'+anchor+'/oracle/java-constants.tsv` |\n'
        text += '| Rust 1.99及1.85 | 各369普通测试／81 harness／4文档通过，143 ignored | `'+anchor+'/verification/rust-commands.json` |\n'
        text += '| 负控 | 6个真实运行时断言失败＋4差分反例拒绝 | `'+anchor+'/result.json` |\n'
        text += '| 原实现保护 | 原338 Rust rs/toml不变；185 registry依赖闭包不变 | `'+anchor+'/result.json` |\n'
        text += '\n- SQLParsingException、SqlSequenceExpr 标为 PARTIAL：Option<String>无法表达Java孤立代理码元字符串，未伪报完整输入域兼容。\n'
        text += '- 23源案例与7个Rust补充测试分开计数；新增30测试未替代完整76分母。\n'
        text += '- 初始RED为E0432结构缺失；运行时RED来自隔离D盘副本中的6个错误实现，两个概念分开记录。\n'
        text += '- 默认及all-feature check、fmt、Clippy -D warnings均已执行；143个ignored和本轮未重跑的数据库现场不宣称通过。\n'
        text += '- 最终catalog：`'+sha(gate/'catalog.json')+'`；初始冻结文档随gate输入保留，本节是验证后统一状态。\n'
        path.write_text(text, 'utf-8')
    state = result | dict(gate=str(gate.relative_to(ROOT)), catalog_sha256=sha(gate/'catalog.json'),
        source_methods_MATCH_across_selected_modules=23, source_methods_MISSING_across_selected_modules=1112,
        concrete_cases_MATCH_across_selected_modules=23, concrete_cases_MISSING_across_selected_modules=1304,
        current_inventory='docs/phase-2/sqlparser_core/inventory.json',
        source_parity='docs/phase-2/sqlparser_core/source-test-parity.json',
        predecessor_baseline='docs/phase-2/wp05-source-junit.json',
        current_specification_state='WP05_IMPLEMENTING; SQLPARSER_CORE_PARTIAL',
        next_action='Migrate shared ColumnMeta/IndexMeta/TableMeta contracts and remaining 53 source cases')
    save(ROOT/'docs/phase-2/wp05-sqlparser-core-values.json', state)
    readme = '# SQL parser core 基础对象迁移\n\n'
    readme += '状态：首个依赖阶段已验证，完整模块未完成。现有 WP05/ADR 是唯一规格事实源。\n\n'
    readme += '- Java固定 `'+inventory['java_pin']+'`，Rust基线 `'+inventory['rust_pin']+'`，未提交既有工作保留。\n'
    readme += '- 新增11对象；9 IMPLEMENTED、2 PARTIAL、20 MISSING；完整分母31对象／76源案例。\n'
    readme += '- 原JUnit76通过；本批23原案例双Rust差分 MATCH，完整88常量匹配。\n'
    readme += '- Rust1.99/1.85各369普通测试、81 harness、4文档通过；143 ignored，数据库现场本轮未重跑。\n'
    readme += '- 6运行时错误实现、4差分反例、13原JUnit事件反例均拒绝。初始编译RED不冒充行为RED。\n'
    readme += '- Java孤立代理码元String域未覆盖，两个字符串对象保持PARTIAL；共享锁延期、TC安全、WP05/WP06/V1仍未完成。\n\n'
    readme += '[机器状态](../../wp05-sqlparser-core-values.json) · [完整迁移路线](../../sqlparser_core/迁移路线图.md)\n\n'
    readme += '最终root：`'+gate.name+'`；catalog `'+sha(gate/'catalog.json')+'`，441成员。\n'
    readme += '执行命令与退出码：[完整记录]('+gate.name+'/verification/rust-commands.json)。\n'
    readme += '历史MSRV观察器Clippy和首次保守mutation判定失败保留为诊断，只有最终成功run被提升。\n'
    (gate.parent/'README.md').write_text(readme, 'utf-8')
    marker = '## 2026-11-18：SQL parser core基础值对象首批迁移'
    progress = ROOT/'docs/delivery-progress.md'
    text = progress.read_text('utf-8')
    need(marker not in text, 'Progress entry already exists')
    progress.write_text(text+'\n\n'+marker+'\n\n'
        '- 在完整31对象／76源案例清单下新增11真实对象；9 IMPLEMENTED、2因Java孤立代理String域PARTIAL、20 MISSING，余53源案例继续实施。\n'
        '- 原Java76测试新跑通过，双Rust23源案例逐项MATCH、88常量匹配；各369普通／81 harness／4 doc通过、143 ignored，6运行时mutation／4差分／13JUnit反例拒绝。338原Rust文件及185 registry依赖闭包不变，DB现场本轮不重跑。\n'
        '- [首批验收](phase-2/evidence/sqlparser-core-values-gate/README.md)、[当前状态](phase-2/wp05-sqlparser-core-values.json)。root '+gate.name+'，catalog '+sha(gate/'catalog.json')+'。\n'
        '- 上一原测试基线的0 MATCH已推进为23 MATCH；两个选定源模块仍1112方法／1304具体案例未迁移。完整core、公开业务COMMIT、原TC安全、WP05/WP06/V1继续OPEN，共享锁延期、外部TC风险未接受。\n', 'utf-8')
    print(json.dumps(dict(published_state='docs/phase-2/wp05-sqlparser-core-values.json', objects=inventory['object_states'], MATCH=23, MISSING=53)), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--gate', type=Path, required=True)
    publish(parser.parse_args().gate)
