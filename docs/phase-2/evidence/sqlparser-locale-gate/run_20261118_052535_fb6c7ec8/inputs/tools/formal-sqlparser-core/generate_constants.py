"""Emit reviewed source constant tables, preserving each original symbol and number."""
import re
from pathlib import Path
from freeze import PACKAGE, ROOT

TRANSLATIONS = {
    'SELECT': '查询', 'INSERT': '插入', 'UPDATE': '更新', 'DELETE': '删除',
    'SELECT_FOR_UPDATE': '锁定查询', 'REPLACE': '替换', 'TRUNCATE': '清空表',
    'CREATE': '创建', 'DROP': '删除对象', 'LOAD': '加载', 'MERGE': '合并',
    'SHOW': '显示', 'ALTER': '修改对象', 'RENAME': '重命名', 'DUMP': '导出',
    'DEBUG': '调试', 'EXPLAIN': '解释计划', 'PROCEDURE': '存储过程',
    'DESC': '描述对象', 'SELECT_LAST_INSERT_ID': '查询最后插入标识',
    'SELECT_WITHOUT_TABLE': '无表查询', 'CREATE_SEQUENCE': '创建序列',
    'SHOW_SEQUENCES': '显示序列', 'GET_SEQUENCE': '获取序列',
    'ALTER_SEQUENCE': '修改序列', 'DROP_SEQUENCE': '删除序列',
    'TDDL_SHOW': 'TDDL显示', 'SET': '设置', 'RELOAD': '重新加载',
    'SELECT_UNION': '联合查询', 'CREATE_TABLE': '创建表', 'DROP_TABLE': '删除表',
    'ALTER_TABLE': '修改表', 'SAVE_POINT': '保存点',
    'SELECT_FROM_UPDATE': '更新返回查询', 'MULTI_DELETE': '多表删除',
    'MULTI_UPDATE': '多表更新', 'CREATE_INDEX': '创建索引', 'DROP_INDEX': '删除索引',
    'KILL': '终止', 'RELEASE_DBLOCK': '释放数据库锁', 'LOCK_TABLES': '锁表',
    'UNLOCK_TABLES': '解锁表', 'CHECK_TABLE': '检查表',
    'SELECT_FOUND_ROWS': '查询匹配行数', 'INSERT_IGNORE': '忽略冲突插入',
    'INSERT_ON_DUPLICATE_UPDATE': '主键冲突时更新插入', 'UPDATE_JOIN': '连接更新',
    'PRIMARY': '主键索引', 'NORMAL': '普通索引', 'UNIQUE': '唯一索引', 'FULL_TEXT': '全文索引'}


def emit_enum(java, target, name, fqcn):
    source = (PACKAGE/java).read_text('utf-8')
    entries = re.findall(r'^\s+([A-Z][A-Z_]+)\((\d+)\)[,;]', source, re.M)
    assert len(entries) == (48 if name == 'SQLType' else 4)
    lines = ['use std::fmt;', 'use crate::InvalidEnumValue;', '',
             f'/// 完整的{name}分类及原始编号，不表示相应SQL已获准执行。',
             f'/// 对应 Java: {fqcn}', '#[allow(non_camel_case_types)]',
             '#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]', '#[repr(i32)]',
             f'pub enum {name} {{']
    for symbol, value in entries:
        lines += [f'    /// {TRANSLATIONS[symbol]}，原编号{value}。', f'    {symbol} = {value},']
    lines += ['}', '', f'impl {name} {{', '    /// 按原 Java 声明顺序排列的完整分类，等同 values()。',
              f'    pub const ALL: [Self; {len(entries)}] = [']
    lines += ['        Self::'+symbol+',' for symbol, _ in entries]
    lines += ['    ];', '', '    /// 返回固定 Java 分类编号；无参数。对应 Java: '+name+'#value',
              '    pub const fn value(self) -> i32 {', '        self as i32', '    }', '}', '',
              f'impl TryFrom<i32> for {name} {{', '    type Error = InvalidEnumValue;', '',
              '    /// 用原始整数 i 查询分类；非法值返回保留原消息的错误，不猜测默认分类。',
              '    fn try_from(i: i32) -> Result<Self, Self::Error> {', '        match i {']
    lines += [f'            {value} => Ok(Self::{symbol}),' for symbol, value in entries]
    lines += [f'            _ => Err(InvalidEnumValue::new("{name}", i)),', '        }', '    }', '}', '',
              f'impl fmt::Display for {name} {{', "    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {",
              '        // 原枚举 toString 返回声明名，不能改成数据库SQL拼写。', '        fmt::Debug::fmt(self, f)',
              '    }', '}', '']
    path = ROOT/'crates/seata_sqlparser/src'/target
    assert not path.exists()
    path.write_text('\n'.join(lines), 'utf-8')


if __name__ == '__main__':
    emit_enum('SQLType.java', 'sql_type.rs', 'SQLType', 'org.apache.seata.sqlparser.SQLType')
    emit_enum('struct/IndexType.java', 'struct/index_type.rs', 'IndexType', 'org.apache.seata.sqlparser.struct.IndexType')
    entries = re.findall(r'String (\w+) = "([^"]+)";', (PACKAGE/'util/JdbcConstants.java').read_text('utf-8'))
    assert len(entries) == 34
    lines = ['/// 数据库类型的原始服务名称常量；存在名称不代表已迁移相应驱动。',
             '/// 对应 Java: org.apache.seata.sqlparser.util.JdbcConstants',
             '#[derive(Debug)]', 'pub struct JdbcConstants {', '    _private: (),', '}', '',
             'impl JdbcConstants {']
    for name, value in entries:
        lines += [f'    /// {name} 的数据库类型标识，保留原始拼写。', f'    pub const {name}: &\'static str = "{value}";']
    lines += ['}', '']
    target = ROOT/'crates/seata_sqlparser/src/util/jdbc_constants.rs'
    assert not target.exists()
    target.write_text('\n'.join(lines), 'utf-8')
    print('SQLType=48 IndexType=4 JdbcConstants=34')
