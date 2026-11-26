/// Rust扩展：保留原分词器的UTF16游标、补充平面合并及越界不移动语义。
/// 对应 Java: sun.text.RuleBasedBreakIterator的getCurrent/getNext/getPrevious私有辅助契约。
pub(crate) struct Utf16Cursor<'a> {
    units: &'a [u16],
    pub(crate) position: usize,
}

impl<'a> Utf16Cursor<'a> {
    /// 用units及有效position建立只读游标，返回独立位置状态。
    /// 对应 Java: StringCharacterIterator#setIndex的内部有效位置使用。
    pub(crate) fn new(units: &'a [u16], position: usize) -> Self {
        Self { units, position }
    }
    /// 无参数，返回当前码点；输入末尾使用JDK的U+FFFF结束标记。
    /// 对应 Java: RuleBasedBreakIterator#getCurrent
    pub(crate) fn current(&self) -> u32 {
        let Some(&first) = self.units.get(self.position) else {
            return 0xffff;
        };
        if (0xd800..=0xdbff).contains(&first) {
            if let Some(&second) = self.units.get(self.position + 1) {
                if (0xdc00..=0xdfff).contains(&second) {
                    return 0x10000 + (u32::from(first) - 0xd800) * 0x400 + u32::from(second)
                        - 0xdc00;
                }
            }
        }
        u32::from(first)
    }
    /// 无参数，返回下一个码点位置，最多为输入长度；不修改当前游标。
    /// 对应 Java: RuleBasedBreakIterator#getNextIndex
    pub(crate) fn next_index(&self) -> usize {
        (self.position + if self.current() > 0xffff { 2 } else { 1 }).min(self.units.len())
    }
    /// 无参数，移动到下一码点并返回它；到达末尾时返回U+FFFF且保留原位置。
    /// 对应 Java: RuleBasedBreakIterator#getNext
    pub(crate) fn next(&mut self) -> u32 {
        let next = self.next_index();
        if next >= self.units.len() {
            0xffff
        } else {
            self.position = next;
            self.current()
        }
    }
    /// 无参数，移动到前一码点并返回它；开头返回U+FFFF且不移动。
    /// 对应 Java: RuleBasedBreakIterator#getPrevious
    pub(crate) fn previous(&mut self) -> u32 {
        if self.position == 0 {
            return 0xffff;
        }
        self.position -= 1;
        let current = self.units[self.position];
        if (0xdc00..=0xdfff).contains(&current)
            && self.position > 0
            && (0xd800..=0xdbff).contains(&self.units[self.position - 1])
        {
            self.position -= 1;
            return self.current();
        }
        u32::from(current)
    }
}

#[cfg(test)]
mod tests {
    use super::Utf16Cursor;

    #[test]
    fn previous_preserves_code_point_before_partial_surrogate_position() {
        // 对应固定JDK17 RuleBasedBreakIterator#getPrevious；从代理对中间回退必须返回原高码元。
        // 这是私有游标契约回归，不计作原Seata的76案例或额外Java差分记录。
        let units = [0x41, 0xd800, 0xdc00, 0x42];
        for (start, expected, position) in [
            (0, 0xffff, 0),
            (1, 0x41, 0),
            (2, 0xd800, 1),
            (3, 0x10000, 1),
            (4, 0x42, 3),
        ] {
            let mut cursor = Utf16Cursor::new(&units, start);
            assert_eq!(
                (cursor.previous(), cursor.position),
                (expected, position),
                "start {start}"
            );
        }
    }
}
