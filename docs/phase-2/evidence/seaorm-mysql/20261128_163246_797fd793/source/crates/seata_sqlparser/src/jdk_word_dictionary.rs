use crate::{
    compatibility_data_reader::CompatibilityDataReader,
    jdk_character_categories::JdkCharacterCategories, MetadataError,
};
use std::sync::OnceLock;

/// Rust扩展：固定JDK17泰语词典状态表，只提供原分词所需的只读状态查询。
/// 对应 Java: sun.text.BreakDictionary；不计入原sqlparser对象分母。
pub(crate) struct JdkWordDictionary {
    columns: usize,
    categories: JdkCharacterCategories,
    rows: Vec<u16>,
    flags: Vec<u32>,
    flag_indices: Vec<i16>,
    shifts: Vec<i8>,
    table: Vec<i16>,
    category_flags: Vec<u8>,
}

static DICTIONARY: OnceLock<Result<JdkWordDictionary, MetadataError>> = OnceLock::new();

impl JdkWordDictionary {
    /// 无参数，返回固定词典的共享引用；数据损坏返回明确Rust错误。
    /// 对应 Java: DictionaryBasedBreakIterator初始化的BreakDictionary。
    pub(crate) fn get() -> Result<&'static Self, MetadataError> {
        DICTIONARY
            .get_or_init(Self::read)
            .as_ref()
            .map_err(Clone::clone)
    }
    fn read() -> Result<Self, MetadataError> {
        let mut reader =
            CompatibilityDataReader::new(include_bytes!("data/jdk17_word_th_dictionary_input.bin"));
        let value = Self {
            columns: reader.u32()? as usize,
            categories: JdkCharacterCategories::read(&mut reader)?,
            rows: reader.shorts()?,
            flags: reader.ints()?,
            flag_indices: reader.shorts()?.into_iter().map(|v| v as i16).collect(),
            shifts: reader.bytes()?.into_iter().map(|v| v as i8).collect(),
            table: reader.shorts()?.into_iter().map(|v| v as i16).collect(),
            category_flags: reader.bytes()?,
        };
        reader.finish()?;
        if value.columns == 0
            || value.rows.len() != value.flag_indices.len()
            || value.rows.len() != value.shifts.len()
            || value.category_flags.iter().any(|v| *v > 1)
        {
            return Err(MetadataError::InvalidCompatibilityData("dictionary shape"));
        }
        Ok(value)
    }
    /// 检查category是否属于原词典分类；IGNORE为false，未知分类返回数据错误。
    /// 对应 Java: DictionaryBasedBreakIterator#lookupCategory的categoryFlags检查。
    pub(crate) fn is_dictionary(&self, category: i32) -> Result<bool, MetadataError> {
        if category < 0 {
            return Ok(false);
        }
        self.category_flags
            .get(category as usize)
            .map(|v| *v != 0)
            .ok_or(MetadataError::InvalidCompatibilityData(
                "dictionary category",
            ))
    }
    /// 按row状态和code_point查询下一状态；-1为单词结束，0为非法前缀。
    /// 对应 Java: BreakDictionary#getNextStateFromCharacter
    pub(crate) fn next_state_from_character(
        &self,
        row: i32,
        code_point: u32,
    ) -> Result<i32, MetadataError> {
        self.next_state(row, self.categories.category(code_point))
    }
    /// 按row和column读取逻辑词典单元；保留原压缩位图、位移及有符号short状态。
    /// 对应 Java: BreakDictionary#getNextState
    pub(crate) fn next_state(&self, row: i32, column: i32) -> Result<i32, MetadataError> {
        let row = usize::try_from(row)
            .map_err(|_| MetadataError::InvalidCompatibilityData("dictionary row"))?;
        let column = usize::try_from(column)
            .map_err(|_| MetadataError::InvalidCompatibilityData("dictionary column"))?;
        let flag = *self
            .flag_indices
            .get(row)
            .ok_or(MetadataError::InvalidCompatibilityData(
                "dictionary row bound",
            ))?;
        let populated = if flag < 0 {
            column == usize::from(flag.unsigned_abs())
        } else {
            let bits = self.flags.get(flag as usize + (column >> 5)).ok_or(
                MetadataError::InvalidCompatibilityData("dictionary flag bound"),
            )?;
            bits & (1_u32 << (column & 31)) != 0
        };
        if !populated {
            return Ok(0);
        }
        let shifted = column as isize + isize::from(self.shifts[row]);
        let shifted = usize::try_from(shifted)
            .map_err(|_| MetadataError::InvalidCompatibilityData("dictionary shift"))?;
        let index = usize::from(self.rows[row]) * self.columns + shifted;
        self.table
            .get(index)
            .map(|v| i32::from(*v))
            .ok_or(MetadataError::InvalidCompatibilityData(
                "dictionary cell bound",
            ))
    }
}
