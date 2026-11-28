use crate::InvalidEnumValue;
use std::fmt;

/// 完整的SQLType分类及原始编号，不表示相应SQL已获准执行。
/// 对应 Java: org.apache.seata.sqlparser.SQLType
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum SQLType {
    /// 查询，原编号0。
    SELECT = 0,
    /// 插入，原编号1。
    INSERT = 1,
    /// 更新，原编号2。
    UPDATE = 2,
    /// 删除，原编号3。
    DELETE = 3,
    /// 锁定查询，原编号4。
    SELECT_FOR_UPDATE = 4,
    /// 替换，原编号5。
    REPLACE = 5,
    /// 清空表，原编号6。
    TRUNCATE = 6,
    /// 创建，原编号7。
    CREATE = 7,
    /// 删除对象，原编号8。
    DROP = 8,
    /// 加载，原编号9。
    LOAD = 9,
    /// 合并，原编号10。
    MERGE = 10,
    /// 显示，原编号11。
    SHOW = 11,
    /// 修改对象，原编号12。
    ALTER = 12,
    /// 重命名，原编号13。
    RENAME = 13,
    /// 导出，原编号14。
    DUMP = 14,
    /// 调试，原编号15。
    DEBUG = 15,
    /// 解释计划，原编号16。
    EXPLAIN = 16,
    /// 存储过程，原编号17。
    PROCEDURE = 17,
    /// 描述对象，原编号18。
    DESC = 18,
    /// 查询最后插入标识，原编号19。
    SELECT_LAST_INSERT_ID = 19,
    /// 无表查询，原编号20。
    SELECT_WITHOUT_TABLE = 20,
    /// 创建序列，原编号21。
    CREATE_SEQUENCE = 21,
    /// 显示序列，原编号22。
    SHOW_SEQUENCES = 22,
    /// 获取序列，原编号23。
    GET_SEQUENCE = 23,
    /// 修改序列，原编号24。
    ALTER_SEQUENCE = 24,
    /// 删除序列，原编号25。
    DROP_SEQUENCE = 25,
    /// TDDL显示，原编号26。
    TDDL_SHOW = 26,
    /// 设置，原编号27。
    SET = 27,
    /// 重新加载，原编号28。
    RELOAD = 28,
    /// 联合查询，原编号29。
    SELECT_UNION = 29,
    /// 创建表，原编号30。
    CREATE_TABLE = 30,
    /// 删除表，原编号31。
    DROP_TABLE = 31,
    /// 修改表，原编号32。
    ALTER_TABLE = 32,
    /// 保存点，原编号33。
    SAVE_POINT = 33,
    /// 更新返回查询，原编号34。
    SELECT_FROM_UPDATE = 34,
    /// 多表删除，原编号35。
    MULTI_DELETE = 35,
    /// 多表更新，原编号36。
    MULTI_UPDATE = 36,
    /// 创建索引，原编号37。
    CREATE_INDEX = 37,
    /// 删除索引，原编号38。
    DROP_INDEX = 38,
    /// 终止，原编号39。
    KILL = 39,
    /// 释放数据库锁，原编号40。
    RELEASE_DBLOCK = 40,
    /// 锁表，原编号41。
    LOCK_TABLES = 41,
    /// 解锁表，原编号42。
    UNLOCK_TABLES = 42,
    /// 检查表，原编号43。
    CHECK_TABLE = 43,
    /// 查询匹配行数，原编号44。
    SELECT_FOUND_ROWS = 44,
    /// 忽略冲突插入，原编号101。
    INSERT_IGNORE = 101,
    /// 主键冲突时更新插入，原编号102。
    INSERT_ON_DUPLICATE_UPDATE = 102,
    /// 连接更新，原编号103。
    UPDATE_JOIN = 103,
}

impl SQLType {
    /// 按原 Java 声明顺序排列的完整分类，等同 values()。
    pub const ALL: [Self; 48] = [
        Self::SELECT,
        Self::INSERT,
        Self::UPDATE,
        Self::DELETE,
        Self::SELECT_FOR_UPDATE,
        Self::REPLACE,
        Self::TRUNCATE,
        Self::CREATE,
        Self::DROP,
        Self::LOAD,
        Self::MERGE,
        Self::SHOW,
        Self::ALTER,
        Self::RENAME,
        Self::DUMP,
        Self::DEBUG,
        Self::EXPLAIN,
        Self::PROCEDURE,
        Self::DESC,
        Self::SELECT_LAST_INSERT_ID,
        Self::SELECT_WITHOUT_TABLE,
        Self::CREATE_SEQUENCE,
        Self::SHOW_SEQUENCES,
        Self::GET_SEQUENCE,
        Self::ALTER_SEQUENCE,
        Self::DROP_SEQUENCE,
        Self::TDDL_SHOW,
        Self::SET,
        Self::RELOAD,
        Self::SELECT_UNION,
        Self::CREATE_TABLE,
        Self::DROP_TABLE,
        Self::ALTER_TABLE,
        Self::SAVE_POINT,
        Self::SELECT_FROM_UPDATE,
        Self::MULTI_DELETE,
        Self::MULTI_UPDATE,
        Self::CREATE_INDEX,
        Self::DROP_INDEX,
        Self::KILL,
        Self::RELEASE_DBLOCK,
        Self::LOCK_TABLES,
        Self::UNLOCK_TABLES,
        Self::CHECK_TABLE,
        Self::SELECT_FOUND_ROWS,
        Self::INSERT_IGNORE,
        Self::INSERT_ON_DUPLICATE_UPDATE,
        Self::UPDATE_JOIN,
    ];

    /// 返回固定 Java 分类编号；无参数。对应 Java: SQLType#value
    pub const fn value(self) -> i32 {
        self as i32
    }
}

impl TryFrom<i32> for SQLType {
    type Error = InvalidEnumValue;

    /// 用原始整数 i 查询分类；非法值返回保留原消息的错误，不猜测默认分类。
    fn try_from(i: i32) -> Result<Self, Self::Error> {
        match i {
            0 => Ok(Self::SELECT),
            1 => Ok(Self::INSERT),
            2 => Ok(Self::UPDATE),
            3 => Ok(Self::DELETE),
            4 => Ok(Self::SELECT_FOR_UPDATE),
            5 => Ok(Self::REPLACE),
            6 => Ok(Self::TRUNCATE),
            7 => Ok(Self::CREATE),
            8 => Ok(Self::DROP),
            9 => Ok(Self::LOAD),
            10 => Ok(Self::MERGE),
            11 => Ok(Self::SHOW),
            12 => Ok(Self::ALTER),
            13 => Ok(Self::RENAME),
            14 => Ok(Self::DUMP),
            15 => Ok(Self::DEBUG),
            16 => Ok(Self::EXPLAIN),
            17 => Ok(Self::PROCEDURE),
            18 => Ok(Self::DESC),
            19 => Ok(Self::SELECT_LAST_INSERT_ID),
            20 => Ok(Self::SELECT_WITHOUT_TABLE),
            21 => Ok(Self::CREATE_SEQUENCE),
            22 => Ok(Self::SHOW_SEQUENCES),
            23 => Ok(Self::GET_SEQUENCE),
            24 => Ok(Self::ALTER_SEQUENCE),
            25 => Ok(Self::DROP_SEQUENCE),
            26 => Ok(Self::TDDL_SHOW),
            27 => Ok(Self::SET),
            28 => Ok(Self::RELOAD),
            29 => Ok(Self::SELECT_UNION),
            30 => Ok(Self::CREATE_TABLE),
            31 => Ok(Self::DROP_TABLE),
            32 => Ok(Self::ALTER_TABLE),
            33 => Ok(Self::SAVE_POINT),
            34 => Ok(Self::SELECT_FROM_UPDATE),
            35 => Ok(Self::MULTI_DELETE),
            36 => Ok(Self::MULTI_UPDATE),
            37 => Ok(Self::CREATE_INDEX),
            38 => Ok(Self::DROP_INDEX),
            39 => Ok(Self::KILL),
            40 => Ok(Self::RELEASE_DBLOCK),
            41 => Ok(Self::LOCK_TABLES),
            42 => Ok(Self::UNLOCK_TABLES),
            43 => Ok(Self::CHECK_TABLE),
            44 => Ok(Self::SELECT_FOUND_ROWS),
            101 => Ok(Self::INSERT_IGNORE),
            102 => Ok(Self::INSERT_ON_DUPLICATE_UPDATE),
            103 => Ok(Self::UPDATE_JOIN),
            _ => Err(InvalidEnumValue::new("SQLType", i)),
        }
    }
}

impl fmt::Display for SQLType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 原枚举 toString 返回声明名，不能改成数据库SQL拼写。
        fmt::Debug::fmt(self, f)
    }
}
