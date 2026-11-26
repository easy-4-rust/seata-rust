"""Freeze the full original core denominator before any Rust implementation."""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path('D:/codex-builds/seata_source_junit_20261118_012407_80edc3ff/source')
MODULE = SOURCE / 'sqlparser/seata-sqlparser-core'
PACKAGE = MODULE / 'src/main/java/org/apache/seata/sqlparser'
DOC = ROOT / 'docs/phase-2/sqlparser_core'
JAVA_PIN = 'cba73e180fd8896a99df6415b859d275122feaa7'
RUST_PIN = '130ad634279cef10cd2998fc3bfae82996224d66'
FIRST = {'SQLType', 'SQLParsingException', 'EscapeSymbol', 'SqlParserType',
         'JdbcConstants', 'IndexType', 'SqlSequenceExpr', 'SqlMethodExpr',
         'SqlDefaultExpr', 'Null', 'NotPlaceholderExpr'}


def snake(name):
    return re.sub(r'([a-z0-9])([A-Z])', r'\1_\2',
                  re.sub(r'([A-Z]+)([A-Z][a-z])', r'\1_\2', name)).lower()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def freeze():
    DOC.mkdir(parents=True, exist_ok=False)
    objects = []
    for path in sorted(PACKAGE.rglob('*.java')):
        text = path.read_text('utf-8')
        package = re.search(r'package ([\w.]+);', text).group(1)
        rel = path.relative_to(PACKAGE)
        target = Path('crates/seata_sqlparser/src') / rel.parent / (snake(path.stem) + '.rs')
        # Full literal source accompanies this explicitly lexical signature inventory.
        signatures = [dict(line=text.count('\n', 0, m.start()) + 1,
                           declaration=m.group().strip())
                      for m in re.finditer(r'(?m)^\s*(?:(?:public|protected|default|static|final|private|synchronized)\s+)*[\w<>?,.\[\] ]+\([^;{}]*\)(?:\s+throws [\w., ]+)?\s*(?=[;{])', text)]
        objects.append(dict(java=path.relative_to(SOURCE).as_posix(), java_type=package+'.'+path.stem,
                            rust=target.as_posix(), rust_type=path.stem, sha256=sha(path),
                            source=text, declarations=signatures, state='MISSING',
                            dependency_stage='values' if path.stem in FIRST else 'remaining',
                            reference_nullability='Java references allow null; no @NonNull contract inferred',
                            inventory_method='lexical signatures plus complete literal source; not AST coverage proof'))
    assert len(objects) == 31
    tests = []
    for path in sorted((MODULE/'src/test').rglob('*')):
        if not path.is_file():
            continue
        rel = path.relative_to(MODULE/'src/test')
        target = ROOT/'crates/seata_test/tests/fixtures/sqlparser_core'/rel
        target.parent.mkdir(parents=True, exist_ok=True)
        assert not target.exists()
        target.write_bytes(path.read_bytes())
        assert sha(target) == sha(path)
        text = path.read_text('utf-8') if path.suffix == '.java' else ''
        package = re.search(r'package ([\w.]+);', text)
        methods = re.findall(r'@Test\s+public void (\w+)\(\)', text)
        tests.extend(dict(id=package.group(1)+'.'+path.stem+'#'+method,
                          source=path.relative_to(SOURCE).as_posix(), source_sha256=sha(path),
                          immutable_copy=target.relative_to(ROOT).as_posix(),
                          state='MISSING', differential='NOT_RUN') for method in methods)
    assert len(tests) == 76
    manifest = dict(java_pin=JAVA_PIN, rust_pin=RUST_PIN, audit_date='2026-11-18',
                    retain_segments=1, object_denominator=31, case_denominator=76,
                    source_test_assets=0, source_test_java_files=12,
                    source_authority='../wp05-mysql-at-plan.md and ../source-junit-contract.md',
                    topology='independent synchronous metadata/value/SPI contracts in contained workspace; no DB/runtime dependency',
                    project_test_package='crates/seata_test (snake_case user override; publish=false)',
                    objects=objects, cases=tests, module_complete=False)
    (DOC/'inventory.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2)+'\n', 'utf-8')
    headings = {
        '迁移路线图.md': ('依赖顺序与工作包', [
            '本次授权延续已批准首期范围；不修改 WP05 核心验收标准。',
            '第一依赖阶段实现11个基础值对象及23个原案例。',
            '第二依赖阶段实现 ColumnMeta、IndexMeta、TableMeta、SqlServerTableMeta。',
            '第三依赖阶段实现识别器 trait、参数容器、元数据缓存与默认值能力。',
            '第四依赖阶段实现 EscapeHandler、工厂和 ColumnUtils 的服务加载链。',
            'SPI须按用户要求验证 inventory 自动注册与并发缓存，当前不加入虚假工厂。',
            '每阶段按依赖编辑；全部31对象冻结后统一进行完整模块验收。']),
        '对象级对照表.md': ('对象边界与状态判定', [
            '一个 Java 主对象对应一个真实 Rust 文件。',
            'mod.rs 与 lib.rs 只能包含模块声明和重导出。',
            '类型沿用原 Java 概念名；方法转为 snake_case。',
            '派生 Debug 不代表原 toString 迁移完成。',
            '本阶段不添加未实施的 trait 空实现，也不添加 todo!。',
            'Java 对象分母31；Rust独立错误辅助类型不进入该分母。',
            '完整方法与文档源文本冻结在 inventory.json；词法清单不宣称 AST 完备证明。']),
        '语义迁移对照表.md': ('可观察语义与实现约束', [
            'Java nullable String 映射 Option<String>，缺省 null 与空字符串保持区别。',
            'Java char 映射 u16，保留 UTF-16 单码元，包括孤立代理码元。',
            'Java 重载构造器映射 new/paired、Default 或 from_cause 等明确入口。',
            'Java 异常消息与 cause 分别保留，cause 使用 Arc 保留原身份。',
            'Java RuntimeException 映射 std::error::Error；不引入 panic 或假成功。',
            'SQLType 迁移全部48个值；它不扩展 seata_at 允许执行的SQL集合。',
            'SqlMethodExpr 计数为 i32，保留负数；默认单例计数仍为0。',
            'TableMeta 共享 ColumnMeta 及大小写不敏感有序映射不能简化为独立副本。',
            'TableMeta 原 equals/hashCode 对 caseSensitive 不一致，须单独审查并显式映射。',
            'IndexMeta 原 equals 忽略 cardinality；空 indextype 的异常分支不得悄悄修正。']),
        '对象名称一致性检查.md': ('名称、路径与审计规则', [
            '映射根为 org.apache.seata.sqlparser，移除该根后保留最后一级 Java 子包。',
            '用户 retain_segments=1 覆盖技能默认2；本模块仅 struct/util 两个子包。',
            'struct 是 Rust 关键字：磁盘目录仍为 struct，模块标识使用 r#struct。',
            'SQLType 对应 sql_type.rs；SQLParsingException 对应 sql_parsing_exception.rs。',
            'NotPlaceHolderExprTest 原测试拼写不更改，生产对象仍为 NotPlaceholderExpr。',
            'getX() 映射 x()；setX(x) 映射 set_x(x)，参数意义与原版保持一致。',
            '31个期待路径逐项核对，名字出现、重导出和空文件均不计完成。',
            '名称检查与语义检查分开；静态无报错不自动提升状态。'])}
    for filename, (section, lines) in headings.items():
        body = [f'# SQL parser core：{filename[:-3]}', '', '## 当前契约与基线', '',
                f'- Java基线：`{JAVA_PIN}`。', f'- Rust基线：`{RUST_PIN}`；现有未提交工作保护。',
                '- 最后审计：2026-11-18。', '- 文档状态：IMPLEMENTING；模块迁移未完成。',
                '- 规格事实源：`../wp05-mysql-at-plan.md`、`../source-junit-contract.md`。',
                '- 完整分母：31对象、12测试文件、76案例、0外部测试资产。',
                '- Rust目标：`crates/seata_sqlparser/src`。',
                '- 项目测试包：`crates/seata_test`，不发布。',
                '- 路径规则：`retain_segments=1`，按用户 AGENTS.md。',
                '- 当前快照：31 MISSING；76 NOT_RUN；不继承 Java 通过为 Rust 通过。', '',
                f'## {section}', ''] + ['- '+line for line in lines]
        body += ['', '## 完整对象矩阵', '', '| Java对象 | 期待Rust路径 | 阶段 | 状态 |',
                 '|---|---|---|---|']
        body += ['| '+o['java_type']+' | `'+o['rust']+'` | '+o['dependency_stage']+' | MISSING |' for o in objects]
        body += ['', '## 严格验证与未完成项', '',
                 '- `inventory.json` 保存原源码、哈希、构造器和方法签名及76案例身份。',
                 '- 所有12原测试文件按字节复制到测试包 fixtures，保留Apache许可。',
                 '- 先记录缺失行为的失败证据，再实现并运行目标及受影响回归。',
                 '- Rust1.99 与MSRV1.85分别验证；编译失败不能冒充行为断言失败。',
                 '- 原JUnit执行与Java/Rust逐案例输出比较分别记录。',
                 '- 文件存在、测试数量或编译通过均不单独作为迁移完成证明。',
                 '- MISSING/PARTIAL/UNVERIFIED 阻止完整模块验收。',
                 '- 外部TC风险未接受，共享锁问题仍为延期；完整WP05/WP06/V1未完成。',
                 '- 当前未创建分支、提交、安装工具或初始化另一套规格体系。', '']
        (DOC/filename).write_text('\n'.join(body), 'utf-8')
    print(json.dumps(dict(objects=31, cases=76, copied_test_files=12, docs=str(DOC))))


if __name__ == '__main__':
    freeze()
