"""Translate the frozen ColumnMeta field/accessor family without flattening its object boundary."""
import re
from freeze import PACKAGE, ROOT, snake


def generate():
    source = (PACKAGE/'struct/ColumnMeta.java').read_text('utf-8')
    fields = re.findall(r'private (String|int|boolean|Object) (\w+);', source)
    assert len(fields) == 20
    types = {'String': 'Option<JavaString>', 'int': 'i32', 'boolean': 'bool',
             'Object': 'Option<Arc<dyn JavaObject>>'}
    getter_overrides = {'isAutoincrement': 'is_autoincrement_text', 'isNullAble': 'is_null_able_text'}
    bool_setters = {'isOnUpdate': ('set_on_update', 'on_update'),
                    'isCaseSensitive': ('set_case_sensitive', 'case_sensitive')}
    labels = {'tableCat': '表目录', 'tableSchemaName': '表模式名', 'tableName': '表名',
              'columnName': '列名', 'dataType': 'JDBC类型编号', 'dataTypeName': '数据库类型名',
              'columnSize': '列长度', 'decimalDigits': '小数位数', 'numPrecRadix': '精度基数',
              'nullAble': 'JDBC可空性编号', 'remarks': '备注', 'columnDef': '默认值',
              'sqlDataType': 'SQL类型编号', 'sqlDatetimeSub': '日期时间子类型编号',
              'charOctetLength': '字符字节长度对象', 'ordinalPosition': '列序号',
              'isNullAble': '原可空性文本', 'isAutoincrement': '原自增文本',
              'isOnUpdate': '自动更新标志', 'isCaseSensitive': '大小写敏感标志'}
    lines = ['use std::{any::Any, fmt, sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard}};',
             'use crate::{JavaObject, JavaString, MetadataError};', '',
             '/// 列元数据共享句柄；Clone 保留同一 Java 对象的引用身份和后续修改。',
             '/// 对应 Java: org.apache.seata.sqlparser.struct.ColumnMeta',
             '#[derive(Clone, Default)]', 'pub struct ColumnMeta {',
             '    state: Arc<RwLock<ColumnMetaState>>,', '}', '',
             '/// ColumnMeta 的内部状态，不是独立迁移对象。',
             '#[derive(Clone, Default)]', 'struct ColumnMetaState {']
    lines += [f'    {snake(name)}: {types[kind]},' for kind, name in fields]
    lines += ['}', '', 'impl ColumnMeta {',
              "    fn read(&self) -> Result<RwLockReadGuard<'_, ColumnMetaState>, MetadataError> {",
              '        self.state.read().map_err(|_| MetadataError::LockPoisoned("ColumnMeta"))', '    }', '',
              "    fn write(&self) -> Result<RwLockWriteGuard<'_, ColumnMetaState>, MetadataError> {",
              '        self.state.write().map_err(|_| MetadataError::LockPoisoned("ColumnMeta"))', '    }', '',
              '    /// 返回自增标志；无参数，原文本按大小写不敏感的YES比较，锁中毒返回错误。',
              '    /// 对应 Java: ColumnMeta#isAutoincrement',
              '    pub fn is_autoincrement(&self) -> Result<bool, MetadataError> {',
              '        // 原JDK17对YES的全部等价码元，包含长s U+017F。',
              '        Ok(self.read()?.is_autoincrement.as_ref().is_some_and(|text| matches!(text.as_utf16(), [0x59 | 0x79, 0x45 | 0x65, 0x53 | 0x73 | 0x17f])))',
              '    }', '']
    for kind, name in fields:
        field = snake(name)
        getter = getter_overrides.get(name, field)
        java_getter = name if kind == 'boolean' else 'get'+name[0].upper()+name[1:]
        setter, parameter = bool_setters.get(name, ('set_'+field, field))
        java_setter = 'set'+('OnUpdate' if name == 'isOnUpdate' else 'CaseSensitive' if name == 'isCaseSensitive' else name[0].upper()+name[1:])
        value = 'self.read()?.'+field+('.clone()' if kind in {'String', 'Object'} else '')
        lines += [f'    /// 返回{labels[name]}；无参数，引用值保留null，锁中毒返回错误。',
                  f'    /// 对应 Java: ColumnMeta#{java_getter}',
                  f'    pub fn {getter}(&self) -> Result<{types[kind]}, MetadataError> {{',
                  f'        Ok({value})', '    }', '',
                  f'    /// 用 {parameter} 替换{labels[name]}；成功返回()，锁中毒返回错误。',
                  f'    /// 对应 Java: ColumnMeta#{java_setter}',
                  f'    pub fn {setter}(&self, {parameter}: {types[kind]}) -> Result<(), MetadataError> {{']
        if kind == 'Object':
            lines += ['        // 先释放元数据锁，再释放旧对象，避免用户Drop回调在锁内重入。',
                      '        let previous = {', '            let mut state = self.write()?;',
                      f'            std::mem::replace(&mut state.{field}, {parameter})', '        };', '        drop(previous);']
        else:
            lines += [f'        self.write()?.{field} = {parameter};']
        lines += ['        Ok(())', '    }', '']
    lines += ['    /// 比较可空动态对象other；同一引用立即为true，类型不同为false，读取失败返回错误。',
              '    /// 对应 Java: ColumnMeta#equals；保留原方法other字段先调用equals的方向。',
              '    pub fn java_equals(&self, other: Option<&dyn Any>) -> Result<bool, MetadataError> {',
              '        let Some(other) = other.and_then(|value| value.downcast_ref::<Self>()) else { return Ok(false); };',
              '        if Arc::ptr_eq(&self.state, &other.state) { return Ok(true); }',
              '        let mine = self.read()?.clone();', '        let theirs = other.read()?.clone();',
              '        // 两份快照均已释放锁，用户对象equals可以安全重入元数据访问。']
    for kind, name in fields:
        field = snake(name)
        if kind == 'Object':
            lines += ['        let same_object = match (&theirs.char_octet_length, &mine.char_octet_length) {',
                      '            (None, None) => true,',
                      '            (Some(left), Some(right)) => Arc::ptr_eq(left, right) || left.java_equals(right.as_ref()),',
                      '            _ => false,', '        };', '        if !same_object { return Ok(false); }',
                      '        // Object.equals回调后，重新读取其后字段。',
                      '        let mine = self.read()?.clone();', '        let theirs = other.read()?.clone();']
        else:
            lines += [f'        if theirs.{field} != mine.{field} {{ return Ok(false); }}']
    lines += ['        Ok(true)', '    }', '',
              '    /// 返回Java的字段哈希和；无参数，按i32回绕，读取失败返回错误。',
              '    /// 对应 Java: ColumnMeta#hashCode；不使用Rust随机Hash替代。',
              '    pub fn java_hash_code(&self) -> Result<i32, MetadataError> {',
              '        let state = self.read()?.clone();', '        let mut hash = 0_i32;']
    for kind, name in fields:
        field = 'state.'+snake(name)
        value = (field+'.as_ref().map_or(0, JavaString::java_hash_code)' if kind == 'String' else
                 field+'.as_ref().map_or(0, |value| value.java_hash_code())' if kind == 'Object' else
                 f'if {field} {{ 1231 }} else {{ 1237 }}' if kind == 'boolean' else field)
        lines += [f'        hash = hash.wrapping_add({value});']
        if kind == 'Object':
            lines += ['        // Object.hashCode回调后，重新读取其后字段。', '        let state = self.read()?.clone();']
    lines += ['        Ok(hash)', '    }', '',
              '    /// 返回原字段顺序的完整Java文本；无参数，UTF-16无损，读取失败返回错误。',
              '    /// 对应 Java: ColumnMeta#toString；null对象及其null的toString均输出字面量null。',
              '    pub fn java_to_string(&self) -> Result<JavaString, MetadataError> {',
              '        let state = self.read()?.clone();', '        let mut units = Vec::new();']
    for i, (kind, name) in enumerate(fields):
        prefix = ('ColumnMeta{' if i == 0 else ', ')+name+'='+ ("'" if kind == 'String' else '')
        lines += ['        units.extend('+json_string(prefix)+'.encode_utf16());']
        field = 'state.'+snake(name)
        if kind == 'String':
            lines += [f'        units.extend({field}.as_ref().map_or_else(|| "null".encode_utf16().collect::<Vec<_>>(), |value| value.as_utf16().to_vec()));',
                      '        units.push(39);']
        elif kind == 'Object':
            lines += [f'        units.extend({field}.as_ref().and_then(|value| value.java_to_string()).map_or_else(|| "null".encode_utf16().collect::<Vec<_>>(), |value| value.as_utf16().to_vec()));',
                      '        // Object.toString回调后，重新读取其后字段。', '        let state = self.read()?.clone();']
        else:
            lines += [f'        units.extend({field}.to_string().encode_utf16());']
    lines += ['        units.push(125);', '        Ok(JavaString::from_utf16(units))', '    }', '}', '',
              'impl fmt::Debug for ColumnMeta {', "    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {",
              '        // 不在锁内调用用户对象的Debug。',
              '        f.debug_struct("ColumnMeta").field("identity", &Arc::as_ptr(&self.state)).finish()',
              '    }', '}', '']
    target = ROOT/'crates/seata_sqlparser/src/struct/column_meta.rs'
    assert not target.exists()
    target.write_text('\n'.join(lines), 'utf-8')
    print('ColumnMeta fields/accessors: '+str(len(fields)))


def json_string(value):
    import json
    return json.dumps(value, ensure_ascii=False)


if __name__ == '__main__':
    generate()
