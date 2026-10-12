//! Manuscript buffer: a rope-backed text store with a character-index
//! cursor, selection, snapshot undo/redo, and a UI-agnostic word-wrap
//! engine (visual rows measured through a caller-provided width closure).
//!
//! v1 notes: undo stores whole-text snapshots (fine for small docs;
//! incremental history comes with the revision system). No scrolling yet —
//! the whole buffer renders.

use ropey::Rope;

const UNDO_CAP: usize = 200;

/// Widest byte prefix of `text` fitting `x` px (char-boundaried).
/// Used for click positioning and visual up/down.
pub fn byte_for_x(text: &str, x: f32, width_of: &mut dyn FnMut(&str) -> f32) -> usize {
    if x <= 0.0 || text.is_empty() {
        return 0;
    }
    if width_of(text) <= x {
        return text.len();
    }
    let mut lo = 0usize;
    let mut hi = text.len();
    while lo < hi {
        let mut mid = (lo + hi + 1) / 2;
        while mid > lo && !text.is_char_boundary(mid) {
            mid -= 1;
        }
        if mid <= lo {
            break;
        }
        if width_of(&text[..mid]) <= x {
            lo = mid;
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    lo
}

/// One wrapped visual row: byte range within a buffer line. Rows cover every
/// byte exactly once, except a single space skipped at soft breaks (a cursor
/// byte there unambiguously ends the previous row).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualRow {
    pub buffer_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Greedily wrap one buffer line (no line endings) into visual rows that fit
/// `measure` px. `width_of` measures any substring. Prefers breaking before
/// the last space; falls back to mid-word breaks on char boundaries.
pub fn wrap_line(
    buffer_line: usize,
    text: &str,
    measure: f32,
    width_of: &mut dyn FnMut(&str) -> f32,
) -> Vec<VisualRow> {
    if text.is_empty() {
        return vec![VisualRow { buffer_line, start_byte: 0, end_byte: 0 }];
    }
    let mut rows = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let rest = &text[start..];
        if width_of(rest) <= measure {
            rows.push(VisualRow { buffer_line, start_byte: start, end_byte: text.len() });
            break;
        }
        // Widest fitting byte prefix, floored to a char boundary.
        let mut lo = 0usize;
        let mut hi = rest.len();
        while lo < hi {
            let mut mid = (lo + hi + 1) / 2;
            while mid > lo && !rest.is_char_boundary(mid) {
                mid -= 1;
            }
            if mid <= lo {
                break;
            }
            if width_of(&rest[..mid]) <= measure {
                lo = mid;
            } else {
                hi = mid.saturating_sub(1);
            }
        }
        if lo == 0 {
            // Even one char overflows: force a single char of progress.
            lo = rest.char_indices().nth(1).map(|(b, _)| b).unwrap_or(rest.len());
        }
        // Prefer breaking before the last space (skip it); else mid-word.
        let (end, next) = match rest[..lo].rfind(' ') {
            Some(s) if s > 0 => (start + s, start + s + 1),
            _ => (start + lo, start + lo),
        };
        rows.push(VisualRow { buffer_line, start_byte: start, end_byte: end });
        start = next.max(start + 1);
    }
    rows
}

pub struct EditorState {
    text: Rope,
    /// Cursor as a character index into the whole buffer (selection head).
    cursor: usize,
    /// Selection anchor (char index), if any. Head is always `cursor`.
    sel_anchor: Option<usize>,
    undo: Vec<String>,
    redo: Vec<String>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            cursor: 0,
            sel_anchor: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn len_chars(&self) -> usize {
        self.text.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.text.len_lines()
    }

    /// Raw cursor index (used by tests and future mouse handling).
    #[allow(dead_code)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    // ----- selection -----
    /// Normalized (min, max) selected char range, if any.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.sel_anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// Drop any selection (used by future mouse handling).
    #[allow(dead_code)]
    pub fn collapse(&mut self) {
        self.sel_anchor = None;
    }

    pub fn select_all(&mut self) {
        self.sel_anchor = Some(0);
        self.cursor = self.len_chars();
    }

    pub fn prepare_extend(&mut self) {
        if self.sel_anchor.is_none() {
            self.sel_anchor = Some(self.cursor);
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection_range()?;
        Some(self.text.slice(a..b).chars().collect())
    }

    /// Remove the selected range without committing (callers commit first).
    fn remove_range(&mut self, a: usize, b: usize) {
        self.text.remove(a..b);
        self.cursor = a;
        self.sel_anchor = None;
    }

    pub fn delete_selection(&mut self) {
        if let Some((a, b)) = self.selection_range() {
            self.commit();
            self.remove_range(a, b);
        }
    }

    // ----- editing -----

    fn commit(&mut self) {
        self.undo.push(self.text.to_string());
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.commit();
        if let Some((a, b)) = self.selection_range() {
            self.remove_range(a, b);
        }
        self.text.insert(self.cursor, s);
        self.cursor += s.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.selection_range().is_some() {
            self.commit();
            let (a, b) = self.selection_range().unwrap();
            self.remove_range(a, b);
            return;
        }
        if self.cursor == 0 {
            return;
        }
        self.commit();
        self.cursor -= 1;
        self.text.remove(self.cursor..self.cursor + 1);
    }

    pub fn delete_forward(&mut self) {
        if self.selection_range().is_some() {
            self.commit();
            let (a, b) = self.selection_range().unwrap();
            self.remove_range(a, b);
            return;
        }
        if self.cursor >= self.len_chars() {
            return;
        }
        self.commit();
        self.text.remove(self.cursor..self.cursor + 1);
    }

    // ----- movement (extend = hold selection) -----

    pub fn move_left(&mut self, extend: bool) {
        if !extend {
            if let Some((a, _)) = self.selection_range() {
                self.cursor = a;
                self.sel_anchor = None;
                return;
            }
        } else {
            self.prepare_extend();
        }
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        if !extend {
            if let Some((_, b)) = self.selection_range() {
                self.cursor = b;
                self.sel_anchor = None;
                return;
            }
        } else {
            self.prepare_extend();
        }
        if self.cursor < self.len_chars() {
            self.cursor += 1;
        }
    }

    pub fn line_home(&mut self, extend: bool) {
        if !extend {
            self.sel_anchor = None;
        } else {
            self.prepare_extend();
        }
        let line = self.text.char_to_line(self.cursor);
        self.cursor = self.text.line_to_char(line);
    }

    pub fn line_end(&mut self, extend: bool) {
        if !extend {
            self.sel_anchor = None;
        } else {
            self.prepare_extend();
        }
        let line = self.text.char_to_line(self.cursor);
        self.cursor = self.text.line_to_char(line) + Self::line_chars(&self.text, line);
    }

    /// Place the cursor from a (buffer line, byte offset) pair, e.g. from
    /// visual-row hit testing or vertical movement.
    pub fn set_cursor_line_byte(&mut self, line: usize, byte: usize, extend: bool) {
        if !extend {
            self.sel_anchor = None;
        } else {
            self.prepare_extend();
        }
        let line = line.min(self.len_lines().saturating_sub(1));
        let text = Self::line_string(&self.text, line);
        let byte = byte.min(text.len());
        let mut col = 0;
        for (b, _) in text.char_indices() {
            if b >= byte {
                break;
            }
            col += 1;
        }
        // A byte offset past the last char boundary still counts that char.
        if byte == text.len() {
            col = text.chars().count();
        }
        self.cursor = self.text.line_to_char(line) + col;
    }

    // ----- undo / redo -----

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn restore(&mut self, snapshot: String) {
        let mut rope = Rope::new();
        rope.insert(0, &snapshot);
        self.text = rope;
        self.cursor = self.cursor.min(self.len_chars());
        self.sel_anchor = None;
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.text.to_string());
            self.restore(prev);
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.text.to_string());
            self.restore(next);
        }
    }

    // ----- queries for rendering -----

    /// Number of characters on a buffer line, excluding line endings.
    fn line_chars(text: &Rope, line: usize) -> usize {
        Self::line_string(text, line).chars().count()
    }

    fn line_string(text: &Rope, line: usize) -> String {
        let slice = text.line(line);
        let s: String = slice.chars().collect();
        s.trim_end_matches(['\r', '\n']).to_string()
    }

    /// Plain text of a buffer line, without line endings.
    pub fn line_text(&self, line: usize) -> String {
        Self::line_string(&self.text, line)
    }

    /// Char index where a buffer line starts.
    pub fn line_start_char(&self, line: usize) -> usize {
        self.text.line_to_char(line)
    }

    /// Character count of a buffer line, excluding line endings.
    pub fn line_char_len(&self, line: usize) -> usize {
        Self::line_chars(&self.text, line)
    }

    /// (line, char-column) of the cursor.
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let line = self.text.char_to_line(self.cursor);
        (line, self.cursor - self.text.line_to_char(line))
    }

    /// (line, byte offset within the line-ending-free line) of the cursor.
    pub fn cursor_line_byte(&self) -> (usize, usize) {
        let (line, col) = self.cursor_line_col();
        let text = self.line_text(line);
        (line, Self::col_to_bytes(&text, col))
    }

    /// UTF-8 byte offset of a char column within a (line-ending-free) line.
    pub fn col_to_bytes(line_text: &str, col: usize) -> usize {
        line_text
            .char_indices()
            .nth(col)
            .map(|(b, _)| b)
            .unwrap_or(line_text.len())
    }

    pub fn word_count(&self) -> usize {
        self.text.to_string().split_whitespace().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_moves_cursor_and_counts_words() {
        let mut ed = EditorState::new();
        ed.insert_str("Hello world");
        assert_eq!(ed.cursor(), 11);
        assert_eq!(ed.word_count(), 2);
        assert_eq!(ed.line_text(0), "Hello world");
    }

    #[test]
    fn arrows_cross_line_boundaries() {
        let mut ed = EditorState::new();
        ed.insert_str("ab\ncd");
        assert_eq!(ed.cursor(), 5);
        ed.move_left(false);
        ed.move_left(false);
        assert_eq!(ed.cursor(), 3);
        let (line, col) = ed.cursor_line_col();
        assert_eq!((line, col), (1, 0));
        ed.move_left(false);
        assert_eq!(ed.cursor_line_col(), (0, 2));
    }

    #[test]
    fn selection_extends_and_collapses() {
        let mut ed = EditorState::new();
        ed.insert_str("hello");
        ed.move_left(true);
        ed.move_left(true);
        assert_eq!(ed.selection_range(), Some((3, 5)));
        assert_eq!(ed.selected_text().as_deref(), Some("lo"));
        ed.move_left(false);
        assert_eq!(ed.selection_range(), None);
        assert_eq!(ed.cursor(), 3);
    }

    #[test]
    fn typing_replaces_selection_in_one_undo() {
        let mut ed = EditorState::new();
        ed.insert_str("hello");
        ed.line_home(false);
        ed.move_right(true);
        ed.move_right(true);
        ed.insert_str("HE");
        assert_eq!(ed.line_text(0), "HEllo");
        ed.undo();
        assert_eq!(ed.line_text(0), "hello");
    }

    #[test]
    fn backspace_joins_lines_and_undo_restores() {
        let mut ed = EditorState::new();
        ed.insert_str("ab\ncd");
        ed.line_home(false);
        ed.backspace();
        assert_eq!(ed.line_text(0), "abcd");
        assert!(ed.can_undo());
        ed.undo();
        assert_eq!(ed.len_lines(), 2);
        assert_eq!(ed.line_text(1), "cd");
        assert!(ed.can_redo());
        ed.redo();
        assert_eq!(ed.line_text(0), "abcd");
    }

    #[test]
    fn wrap_breaks_before_spaces() {
        let widths = |s: &str| s.len() as f32 * 10.0;
        let rows = wrap_line(0, "aa bb cc", 45.0, &mut widths);
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[0].start_byte, rows[0].end_byte), (0, 2));
        assert_eq!((rows[1].start_byte, rows[1].end_byte), (3, 5));
        assert_eq!((rows[2].start_byte, rows[2].end_byte), (6, 8));
    }

    #[test]
    fn wrap_breaks_mid_word_without_spaces() {
        let widths = |s: &str| s.len() as f32 * 10.0;
        let rows = wrap_line(0, "abcdefgh", 35.0, &mut widths);
        assert!(rows.len() >= 2);
        // Full byte coverage, no gaps or overlaps.
        assert_eq!(rows[0].start_byte, 0);
        assert_eq!(rows.last().unwrap().end_byte, 8);
        for pair in rows.windows(2) {
            assert!(pair[1].start_byte >= pair[0].end_byte);
        }
    }

    #[test]
    fn wrap_empty_line_is_one_row() {
        let rows = wrap_line(3, "", 100.0, &mut |_| 0.0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].buffer_line, 3);
    }

    #[test]
    fn byte_offsets_are_char_boundaries() {
        let mut ed = EditorState::new();
        ed.insert_str("héllo");
        let line = ed.line_text(0);
        assert_eq!(EditorState::col_to_bytes(&line, 1), 1);
        assert_eq!(EditorState::col_to_bytes(&line, 2), 3);
        assert_eq!(EditorState::col_to_bytes(&line, 99), line.len());
    }
}
