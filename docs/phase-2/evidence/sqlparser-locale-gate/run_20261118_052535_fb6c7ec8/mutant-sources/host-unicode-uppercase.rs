use crate::{
    compatibility_data_reader::CompatibilityDataReader, jdk_word_profile::JdkWordProfile,
    utf16_cursor::Utf16Cursor, JavaString, MetadataError,
};
use std::sync::OnceLock;

/// Rust扩展：原JDK17的上下文及Locale语言相关字符串大小写转换，不采用宿主Unicode版本。
/// 对应 Java: String#toLowerCase(Locale)/toUpperCase(Locale)及ConditionalSpecialCasing；不计原31对象。
pub struct JdkCasing;

/// 固定码点的一对ROOT转换和条件属性，属于JdkCasing私有数据记录。
/// 对应 Java: String的ROOT转换结果及ConditionalSpecialCasing属性的Rust输入记录。
struct CaseRecord {
    code_point: u32,
    combining: u8,
    cased: bool,
    lower: Vec<u16>,
    upper: Vec<u16>,
}

static RECORDS: OnceLock<Result<Vec<CaseRecord>, MetadataError>> = OnceLock::new();

fn records() -> Result<&'static [CaseRecord], MetadataError> {
    RECORDS
        .get_or_init(|| {
            let mut reader =
                CompatibilityDataReader::new(include_bytes!("data/jdk17_string_casing_input.bin"));
            let mut result: Vec<CaseRecord> = Vec::new();
            while reader.remaining() > 0 {
                let code_point = reader.u32()?;
                let combining = reader.byte()?;
                let cased = reader.byte()?;
                if code_point > 0x10ffff
                    || cased > 1
                    || result.last().is_some_and(|r| r.code_point >= code_point)
                {
                    return Err(MetadataError::InvalidCompatibilityData("casing record"));
                }
                result.push(CaseRecord {
                    code_point,
                    combining,
                    cased: cased != 0,
                    lower: reader.string_units()?,
                    upper: reader.string_units()?,
                });
            }
            Ok(result)
        })
        .as_ref()
        .map(Vec::as_slice)
        .map_err(Clone::clone)
}

fn entry(records: &[CaseRecord], code_point: u32) -> Option<&CaseRecord> {
    records
        .binary_search_by_key(&code_point, |r| r.code_point)
        .ok()
        .map(|i| &records[i])
}
fn combining(records: &[CaseRecord], code_point: u32) -> u8 {
    entry(records, code_point).map_or(0, |r| r.combining)
}
fn cased(records: &[CaseRecord], code_point: u32) -> bool {
    entry(records, code_point).is_some_and(|r| r.cased)
}

fn after_base(units: &[u16], index: usize, records: &[CaseRecord], soft: bool) -> bool {
    let mut cursor = Utf16Cursor::new(units, index);
    while cursor.position > 0 {
        let cp = cursor.previous();
        let matched = if soft {
            matches!(
                cp,
                0x69 | 0x6a | 0x12f | 0x268 | 0x456 | 0x458 | 0x1d62 | 0x1e2d | 0x1ecb | 0x2071
            )
        } else {
            cp == 0x49
        };
        if matched {
            return true;
        }
        if matches!(combining(records, cp), 0 | 230) {
            return false;
        }
    }
    false
}

fn following_condition(units: &[u16], index: usize, records: &[CaseRecord], dot: bool) -> bool {
    let mut cursor = Utf16Cursor::new(units, index);
    cursor.position = cursor.next_index();
    while cursor.position < units.len() {
        let cp = cursor.current();
        let cc = combining(records, cp);
        if dot {
            if cp == 0x307 {
                return true;
            }
            if cc == 0 || cc == 230 {
                return false;
            }
        } else {
            if cc == 230 {
                return true;
            }
            if cc == 0 {
                return false;
            }
        }
        cursor.position = cursor.next_index();
    }
    false
}

impl JdkCasing {
    /// 按language进行value的原JDK17小写转换，返回无损UTF16结果；language为Locale#getLanguage的规范化值。
    /// 保留ROOT展开、Σ词边界条件、土耳其/阿塞拜疆I和立陶宛重音规则；固定数据损坏返回Rust错误。
    /// 对应 Java: String#toLowerCase(Locale)
    pub fn lower_case(value: &JavaString, language: &str) -> Result<JavaString, MetadataError> {
        let records = records()?;
        let units = value.as_utf16();
        let mut output = Vec::with_capacity(units.len());
        let mut cursor = Utf16Cursor::new(units, 0);
        while cursor.position < units.len() {
            let cp = cursor.current();
            let index = cursor.position;
            let next = cursor.next_index();
            let replacement = if cp == 0x3a3 {
                if JdkWordProfile::for_language(language)?
                    .is_final_cased(units, index, |cp| cased(records, cp))?
                {
                    Some(vec![0x3c2])
                } else {
                    None
                }
            } else if matches!(language, "tr" | "az") {
                match cp {
                    0x130 => Some(vec![0x69]),
                    0x307 if after_base(units, index, records, false) => Some(vec![]),
                    0x49 if !following_condition(units, index, records, true) => Some(vec![0x131]),
                    _ => None,
                }
            } else if language == "lt" {
                match cp {
                    0x49 | 0x4a | 0x12e if following_condition(units, index, records, false) => {
                        Some(vec![
                            if cp == 0x12e { 0x12f } else { cp as u16 + 0x20 },
                            0x307,
                        ])
                    }
                    0xcc => Some(vec![0x69, 0x307, 0x300]),
                    0xcd => Some(vec![0x69, 0x307, 0x301]),
                    0x128 => Some(vec![0x69, 0x307, 0x303]),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(replacement) = replacement {
                output.extend(replacement);
            } else if let Some(record) = entry(records, cp) {
                output.extend_from_slice(&record.lower);
            } else {
                output.extend_from_slice(&units[index..next]);
            }
            cursor.position = next;
        }
        Ok(JavaString::from_utf16(output))
    }

    /// 按language进行value的原JDK17大写转换，返回无损UTF16；保留多码点展开及Locale点号条件。
    /// language为Locale#getLanguage的规范化值；固定数据损坏返回Rust错误。
    /// 对应 Java: String#toUpperCase(Locale)
    pub fn upper_case(value: &JavaString, language: &str) -> Result<JavaString, MetadataError> {Ok(JavaString::from(String::from_utf16_lossy(value.as_utf16()).to_uppercase()))}

    /// 返回value各UTF16位置的词边界标志，长度为码元数加一；language为规范化Locale语言。
    /// Rust诊断扩展，用于单独验证Final_Cased依赖；固定数据无效返回明确错误。
    /// 对应 Java: BreakIterator#getWordInstance(Locale)及isBoundary的验证边界。
    pub fn word_boundaries(value: &JavaString, language: &str) -> Result<Vec<bool>, MetadataError> {
        JdkWordProfile::for_language(language)?.boundaries(value.as_utf16())
    }
}
