use crate::{
    compatibility_data_reader::CompatibilityDataReader,
    jdk_character_categories::JdkCharacterCategories, jdk_word_dictionary::JdkWordDictionary,
    utf16_cursor::Utf16Cursor, MetadataError,
};
use std::sync::OnceLock;

/// Rust扩展：固定JDK17词边界状态机，供String大小写的Final_Cased条件使用。
/// 对应 Java: sun.text.RuleBasedBreakIterator及DictionaryBasedBreakIterator的词边界契约。
pub(crate) struct JdkWordProfile {
    columns: usize,
    states: Vec<u16>,
    backwards: Vec<u16>,
    end: Vec<u8>,
    lookahead: Vec<u8>,
    categories: JdkCharacterCategories,
    thai: bool,
}

static ROOT: OnceLock<Result<JdkWordProfile, MetadataError>> = OnceLock::new();
static THAI: OnceLock<Result<JdkWordProfile, MetadataError>> = OnceLock::new();

impl JdkWordProfile {
    /// 按JDK规范化language选择词边界配置并返回共享引用；未知语言保留原ROOT回退。
    /// 对应 Java: BreakIterator#getWordInstance(Locale)
    pub(crate) fn for_language(language: &str) -> Result<&'static Self, MetadataError> {
        let (slot, bytes, thai) = if language == "th" {
            (
                &THAI,
                include_bytes!("data/jdk17_word_th_input.bin").as_slice(),
                true,
            )
        } else {
            (
                &ROOT,
                include_bytes!("data/jdk17_word_root_input.bin").as_slice(),
                false,
            )
        };
        slot.get_or_init(|| Self::read(bytes, thai))
            .as_ref()
            .map_err(Clone::clone)
    }
    fn read(bytes: &[u8], thai: bool) -> Result<Self, MetadataError> {
        let mut reader = CompatibilityDataReader::new(bytes);
        let columns = reader.u32()? as usize;
        let states = reader.shorts()?;
        let backwards = reader.shorts()?;
        let end = reader.bytes()?;
        let lookahead = reader.bytes()?;
        let categories = JdkCharacterCategories::read(&mut reader)?;
        reader.finish()?;
        if columns == 0
            || states.len() != columns * end.len()
            || end.len() != lookahead.len()
            || states.iter().any(|v| usize::from(*v) >= end.len())
            || end.iter().chain(&lookahead).any(|v| *v > 1)
        {
            return Err(MetadataError::InvalidCompatibilityData(
                "word profile shape",
            ));
        }
        Ok(Self {
            columns,
            states,
            backwards,
            end,
            lookahead,
            categories,
            thai,
        })
    }
    fn previous_safe(&self, cursor: &mut Utf16Cursor<'_>) -> Result<usize, MetadataError> {
        let mut state = 1;
        let mut category = 0;
        let mut last_category = 0;
        let mut cp = cursor.current();
        while cp != 0xffff && state != 0 {
            last_category = category;
            category = self.categories.category(cp);
            if category >= 0 {
                state = usize::from(
                    *self
                        .backwards
                        .get(state * self.columns + category as usize)
                        .ok_or(MetadataError::InvalidCompatibilityData(
                            "backward word state bound",
                        ))?,
                );
            }
            cp = cursor.previous();
        }
        if cp != 0xffff {
            cursor.next();
            if last_category != -1 {
                cursor.next();
            }
        }
        Ok(cursor.position)
    }
    fn next_range(
        &self,
        units: &[u16],
        start: usize,
        dictionary: Option<&JdkWordDictionary>,
    ) -> Result<(usize, usize), MetadataError> {
        let mut cursor = Utf16Cursor::new(units, start);
        let mut result = cursor.next_index();
        let mut lookahead_result = 0;
        let mut state = 1;
        let mut cp = cursor.current();
        let mut dictionary_count = 0;
        while cp != 0xffff && state != 0 {
            let category = self.categories.category(cp);
            if let Some(dictionary) = dictionary {
                if dictionary.is_dictionary(category)? {
                    dictionary_count += 1;
                }
            }
            if category >= 0 {
                state = usize::from(
                    *self
                        .states
                        .get(state * self.columns + category as usize)
                        .ok_or(MetadataError::InvalidCompatibilityData("word state bound"))?,
                );
            }
            if self.lookahead[state] != 0 {
                if self.end[state] != 0 {
                    result = lookahead_result;
                } else {
                    lookahead_result = cursor.next_index();
                }
            } else if self.end[state] != 0 {
                result = cursor.next_index();
            }
            cp = cursor.next();
        }
        if cp == 0xffff && lookahead_result == units.len() {
            result = lookahead_result;
        }
        if result <= start || result > units.len() {
            return Err(MetadataError::InvalidCompatibilityData(
                "word boundary progress",
            ));
        }
        Ok((result, dictionary_count))
    }
    fn divide_dictionary(
        &self,
        units: &[u16],
        start: usize,
        end: usize,
        dictionary: &JdkWordDictionary,
    ) -> Result<Vec<usize>, MetadataError> {
        let mut cursor = Utf16Cursor::new(units, start);
        while !dictionary.is_dictionary(self.categories.category(cursor.current()))? {
            cursor.next();
        }
        let mut current: Vec<usize> = Vec::new();
        let mut possible = Vec::new();
        let mut wrong = Vec::new();
        let mut state = 0;
        let mut farthest = cursor.position;
        let mut best: Option<Vec<usize>> = None;
        let mut cp = cursor.current();
        loop {
            if dictionary.next_state(state, 0)? == -1 {
                possible.push(cursor.position);
            }
            state = dictionary.next_state_from_character(state, cp)?;
            if state == -1 {
                current.push(cursor.position);
                break;
            }
            if state == 0 || cursor.position >= end {
                if cursor.position > farthest {
                    farthest = cursor.position;
                    best = Some(current.clone());
                }
                while possible.last().is_some_and(|p| wrong.contains(p)) {
                    possible.pop();
                }
                if possible.is_empty() {
                    if let Some(best) = &best {
                        // 回退到farthest+1后，下次错误先重新复制best；按值复制与原Stack别名可观察等价。
                        current = best.clone();
                        if farthest < end {
                            cursor.position = farthest + 1;
                        } else {
                            break;
                        }
                    } else {
                        if current.last() != Some(&cursor.position) && cursor.position != start {
                            current.push(cursor.position);
                        }
                        cursor.next();
                        current.push(cursor.position);
                    }
                } else if let Some(candidate) = possible.pop() {
                    while current.last().is_some_and(|p| candidate < *p) {
                        if let Some(removed) = current.pop() {
                            wrong.push(removed);
                        }
                    }
                    current.push(candidate);
                    cursor.position = candidate;
                }
                cp = cursor.current();
                if cursor.position >= end {
                    break;
                }
            } else {
                cp = cursor.next();
            }
        }
        current.pop();
        current.push(end);
        let mut result = Vec::with_capacity(current.len() + 1);
        result.push(start);
        result.extend(current);
        Ok(result)
    }
    /// 返回units全部码元位置的词边界标志；返回值长度为units长度加一。
    /// 对应 Java: BreakIterator词边界位置，用于ConditionalSpecialCasing的Final_Cased条件。
    pub(crate) fn boundaries(&self, units: &[u16]) -> Result<Vec<bool>, MetadataError> {
        let mut iterator = WordIterator::new(self, units)?;
        (0..=units.len())
            .map(|index| iterator.is_boundary(index))
            .collect()
    }

    /// 按原查询顺序判断index处Σ的Final_Cased条件，cased查询使用固定JDK属性。
    /// 对应 Java: ConditionalSpecialCasing#isFinalCased，保留随机访问及查询缓存副作用。
    pub(crate) fn is_final_cased(
        &self,
        units: &[u16],
        index: usize,
        cased: impl Fn(u32) -> bool,
    ) -> Result<bool, MetadataError> {let flags=self.boundaries(units)?;let mut before=Utf16Cursor::new(units,index);while !flags[before.position]{let cp=before.previous();if cased(cp){let mut after=Utf16Cursor::new(units,index);after.position=after.next_index();while after.position<units.len()&&!flags[after.position]{if cased(after.current()){return Ok(false);}after.position=after.next_index();}return Ok(true);}}Ok(false)}
}

/// JdkWordProfile私有查询游标；保留原随机访问的安全回退位置和泰语缓存。
/// 对应 Java: RuleBasedBreakIterator随机查询状态及DictionaryBasedBreakIterator缓存。
struct WordIterator<'a> {
    profile: &'a JdkWordProfile,
    units: &'a [u16],
    cursor: Utf16Cursor<'a>,
    dictionary: Option<&'static JdkWordDictionary>,
    cached_break: Option<usize>,
    cached_dictionary: Option<Vec<usize>>,
    position_in_cache: usize,
}

impl<'a> WordIterator<'a> {
    fn new(profile: &'a JdkWordProfile, units: &'a [u16]) -> Result<Self, MetadataError> {
        Ok(Self {
            profile,
            units,
            cursor: Utf16Cursor::new(units, 0),
            dictionary: if profile.thai {
                Some(JdkWordDictionary::get()?)
            } else {
                None
            },
            cached_break: None,
            cached_dictionary: None,
            position_in_cache: 0,
        })
    }
    fn handle_next(&mut self) -> Result<Option<usize>, MetadataError> {
        if self
            .cached_dictionary
            .as_ref()
            .is_none_or(|positions| self.position_in_cache == positions.len() - 1)
        {
            let start = self.cursor.position;
            if start == self.units.len() {
                self.cached_dictionary = None;
                return Ok(None);
            }
            let (end, count) = self
                .profile
                .next_range(self.units, start, self.dictionary)?;
            self.cursor.position = end;
            if count > 1 && end - start > 1 {
                if let Some(dictionary) = self.dictionary {
                    self.cached_dictionary = Some(
                        self.profile
                            .divide_dictionary(self.units, start, end, dictionary)?,
                    );
                    self.position_in_cache = 0;
                }
            } else {
                self.cached_dictionary = None;
                return Ok(Some(end));
            }
        }
        let positions =
            self.cached_dictionary
                .as_ref()
                .ok_or(MetadataError::InvalidCompatibilityData(
                    "word dictionary cache",
                ))?;
        self.position_in_cache += 1;
        let next = *positions.get(self.position_in_cache).ok_or(
            MetadataError::InvalidCompatibilityData("word dictionary cache bound"),
        )?;
        self.cursor.position = next;
        Ok(Some(next))
    }
    fn following(&mut self, offset: usize) -> Result<Option<usize>, MetadataError> {
        if let Some(positions) = &self.cached_dictionary {
            if offset >= positions[0] && offset < positions[positions.len() - 1] {
                self.position_in_cache = 0;
                while self.position_in_cache < positions.len()
                    && offset >= positions[self.position_in_cache]
                {
                    self.position_in_cache += 1;
                }
                let next = positions[self.position_in_cache];
                self.cursor.position = next;
                return Ok(Some(next));
            }
        }
        self.cached_dictionary = None;
        self.cursor.position = offset;
        if offset == 0 {
            let result = self.handle_next()?;
            self.cached_break = result;
            return Ok(result);
        }
        let mut result = self.cached_break;
        if result.is_none_or(|known| known >= offset) {
            result = Some(self.profile.previous_safe(&mut self.cursor)?);
        } else if let Some(result) = result {
            self.cursor.position = result;
        }
        while result.is_some_and(|position| position <= offset) {
            result = self.handle_next()?;
        }
        self.cached_break = result;
        Ok(result)
    }
    fn is_boundary(&mut self, offset: usize) -> Result<bool, MetadataError> {
        if offset == 0 {
            Ok(true)
        } else {
            Ok(self.following(offset - 1)? == Some(offset))
        }
    }
}
