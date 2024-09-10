use core::range::Range;

/// Manages text editing of input fields.

fn len_of_codepoint_on(s: &str, index: usize) -> Option<usize> {
    let byte = *s.as_bytes().get(index)?;
    match byte {
        0b00000000..=0b01111111 => Some(1),
        0b11000000..=0b11011111 => Some(2),
        0b11100000..=0b11101111 => Some(3),
        0b11110000..=0b11110111 => Some(4),
        _ => unreachable!(),
    }
}

/// For text:
/// ```txt
/// ABCDEFG
///    ^
///    | index
/// ```
/// ... where `A`, `B`, `C`, etc. represents possible multi-byte code points, and `index` points to
/// the first byte of `D`.
/// Returns length of `C`.
fn len_of_prev_codepoint(s: &str, index: usize) -> Option<usize> {
    let mut bytes = s.as_bytes().get(..index)?.iter();

    // 0xxxxxxx
    // 110xxxxx 10xxxxxx
    // 1110xxxx 10xxxxxx 10xxxxxx
    // 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx

    let byte0 = *bytes.next_back()?;
    if byte0 < 128 {
        return Some(1);
    }

    let byte1 = bytes.next_back().unwrap();
    if byte1 & 0b11000000 != 0b10000000 {
        return Some(2);
    }

    let byte2 = bytes.next_back().unwrap();
    if byte2 & 0b11000000 != 0b10000000 {
        return Some(3);
    }

    Some(4)
}

/// Move `index` one character forward.
/// Returns `true` if `index` is moved, `false` if not moved because of range.
/// Note `index` can be one-past.
fn index_next(s: &str, index: &mut usize) -> bool {
    let Some(len) = len_of_codepoint_on(s, *index) else {
        return false;
    };
    *index += len;
    true
}

/// Move `index` one character forward.
/// Returns `true` if `index` is moved, `false` if not moved because of range.
/// Note `index` can be one-past.
fn index_prev(s: &str, index: &mut usize) -> bool {
    let Some(len) = len_of_prev_codepoint(s, *index) else {
        return false;
    };
    *index -= len;
    true
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Cursor {
    Caret(usize),
    Selection(Range<usize>),
}

#[derive(Debug, Clone, Default)]
pub struct InputFieldState {
    text: String,
    /// If `caret2` is `Some`, the input field is in selection mode.
    caret: usize,
    /// The end of selection.
    caret2: Option<usize>,
}

impl InputFieldState {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> Cursor {
        match (self.caret, self.caret2) {
            (caret, None) => Cursor::Caret(caret),
            (_, Some(_)) => todo!(),
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.caret = 0;
        self.caret2 = None;
    }

    pub fn take_text(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.caret = 0;
        self.caret2 = None;
        text
    }

    pub fn is_in_selection_mode(&self) -> bool {
        self.caret2.is_some()
    }

    pub fn caret_is_at_end(&self) -> bool {
        self.caret == self.text.len()
    }

    pub fn insert(&mut self, char: char) {
        if self.is_in_selection_mode() {
            todo!()
        }
        self.text.insert(self.caret, char);
        index_next(&self.text, &mut self.caret);
    }

    pub fn delete_backward(&mut self) {
        if self.is_in_selection_mode() {
            todo!()
        }
        index_prev(&self.text, &mut self.caret);
        if self.caret_is_at_end() {
            self.text.pop();
        } else {
            self.text.remove(self.caret);
        }
    }

    pub fn delete_forward(&mut self) {
        if self.is_in_selection_mode() {
            todo!()
        }
        if !self.caret_is_at_end() {
            self.text.remove(self.caret);
        }
    }

    pub fn caret_left(&mut self) {
        index_prev(&self.text, &mut self.caret);
    }

    pub fn caret_right(&mut self) {
        index_next(&self.text, &mut self.caret);
    }

    pub fn caret_left_end(&mut self) {
        if self.is_in_selection_mode() {
            todo!()
        }
        self.caret = 0;
    }

    pub fn caret_right_end(&mut self) {
        if self.is_in_selection_mode() {
            todo!()
        }
        self.caret = self.text.len();
    }

    pub fn select_left(&mut self) {
        todo!()
    }

    pub fn select_right(&mut self) {
        todo!()
    }
}
