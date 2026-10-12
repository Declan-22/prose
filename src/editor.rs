//! Manuscript buffer: a rope-backed text store with a character-index
//! cursor, selection, snapshot undo/redo, and a UI-agnostic word-wrap
//! engine (visual rows measured through a caller-provided width closure).
//!
//! v1 notes: undo stores whole-text snapshots (fine for small docs;
//! incremental history comes with the revision system). No scrolling yet —
//! the whole buffer renders.

use ropey::Rope;

const UNDO_CAP: usize = 200;

/// ATX heading level of a buffer line (0 = body text): counts leading
/// `#`s followed by a space, tab, or end of line. Clamped to 3 visual
/// levels; deeper headings render as level 3.
pub fn heading_level(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut hashes = 0;
    while hashes < bytes.len() && bytes[hashes] == b'#' {
        hashes += 1;
    }
    if hashes == 0 {
        return 0;
    }
    match bytes.get(hashes) {
        Some(b' ') | Some(b'\t') | None => hashes.min(3),
        _ => 0,
    }
}

/// Byte length of the `#... ` heading marker, or 0 when not a heading.
pub fn heading_marker_len(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut hashes = 0;
    while hashes < bytes.len() && bytes[hashes] == b'#' {
        hashes += 1;
    }
    if hashes == 0 || hashes > 6 {
        return 0;
    }
    match bytes.get(hashes) {
        Some(b' ') | Some(b'\t') => hashes + 1,
        None => hashes,
        _ => 0,
    }
}

/// A line of only `---` / `***` / `___` (3+) renders as a horizontal rule.
pub fn is_hr_line(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 3 {
        return false;
    }
    let mut chars = t.chars();
    let first = chars.next().unwrap();
    if first != '-' && first != '*' && first != '_' {
        return false;
    }
    chars.all(|c| c == first)
}

/// Inline span kind. Markers (`**`, backticks, …) are never part of a span —
/// hidden lines skip them, active lines render the raw line instead.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpanKind {
    Plain,
    Marker,
    Bold,
    Italic,
    Strike,
    Code,
}

/// Parse an active source line while keeping Markdown delimiters visible.
/// Delimiters are separate spans so the shell can color them independently.
pub fn parse_active_line(line: &str, marker_len: usize) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let base = marker_len.min(line.len());
    if base > 0 {
        spans.push(InlineSpan { start: 0, end: base, kind: SpanKind::Marker });
    }
    let mut plain_start = base;
    let mut i = base;
    while i < line.len() {
        let rest = &line[i..];
        let opener = if rest.starts_with("**") {
            Some((2, "**", SpanKind::Bold))
        } else if rest.starts_with("__") {
            Some((2, "__", SpanKind::Bold))
        } else if rest.starts_with("~~") {
            Some((2, "~~", SpanKind::Strike))
        } else if rest.starts_with('`') {
            Some((1, "`", SpanKind::Code))
        } else if rest.starts_with('*') && !rest[1..].starts_with('*') {
            Some((1, "*", SpanKind::Italic))
        } else {
            None
        };
        let Some((open_len, marker, kind)) = opener else {
            i += rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            continue;
        };
        let content_start = i + open_len;
        let content = &line[content_start..];
        let close = if marker == "*" {
            find_single_star(content)
        } else {
            content.find(marker)
        };
        let Some(close_rel) = close.filter(|end| *end > 0) else {
            i += open_len;
            continue;
        };
        let close_start = content_start + close_rel;
        let close_end = close_start + marker.len();
        if plain_start < i {
            spans.push(InlineSpan { start: plain_start, end: i, kind: SpanKind::Plain });
        }
        spans.push(InlineSpan { start: i, end: content_start, kind: SpanKind::Marker });
        spans.push(InlineSpan { start: content_start, end: close_start, kind });
        spans.push(InlineSpan { start: close_start, end: close_end, kind: SpanKind::Marker });
        i = close_end;
        plain_start = i;
    }
    if plain_start < line.len() {
        spans.push(InlineSpan { start: plain_start, end: line.len(), kind: SpanKind::Plain });
    }
    spans
}

/// Visible slice of a buffer line, in source byte coordinates.
#[derive(Clone, Copy, Debug)]
pub struct InlineSpan {
    pub start: usize,
    pub end: usize,
    pub kind: SpanKind,
}

/// Parse a line into visible spans. When `hide` is false the whole line is
/// one plain span (raw source view, markers visible). When true, heading
/// markers and inline markup are skipped; unmatched markers stay literal.
pub fn parse_line(line: &str, hide: bool, marker_len: usize) -> Vec<InlineSpan> {
    if !hide {
        return vec![InlineSpan { start: 0, end: line.len(), kind: SpanKind::Plain }];
    }
    let base = marker_len.min(line.len());
    parse_inline(&line[base..], base)
}

fn parse_inline(text: &str, base: usize) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    // Flush pending plain text in [plain_start, i).
    let mut plain_start = 0;
    let mut i = 0;
    let flush_plain = |spans: &mut Vec<InlineSpan>, end: usize, plain_start: &mut usize| {
        if *plain_start < end {
            // Merge with a trailing plain span when adjacent.
            if let Some(last) = spans.last_mut() {
                if last.kind == SpanKind::Plain && last.end == base + *plain_start {
                    last.end = base + end;
                    *plain_start = end;
                    return;
                }
            }
            spans.push(InlineSpan {
                start: base + *plain_start,
                end: base + end,
                kind: SpanKind::Plain,
            });
            *plain_start = end;
        }
    };
    while i < text.len() {
        let rest = &text[i..];
        let opener = if rest.starts_with('`') {
            Some((1, "`", SpanKind::Code, false))
        } else if rest.starts_with("**") {
            Some((2, "**", SpanKind::Bold, false))
        } else if rest.starts_with("~~") {
            Some((2, "~~", SpanKind::Strike, false))
        } else if rest.starts_with("__") {
            Some((2, "__", SpanKind::Bold, false))
        } else if rest.starts_with('*')
            && !rest[1..].starts_with('*')
            && (i == 0 || !text[..i].ends_with('*'))
        {
            Some((1, "*", SpanKind::Italic, true))
        } else {
            None
        };
        match opener {
            Some((open_len, marker, kind, single_star)) => {
                let content = &rest[open_len..];
                let close = if single_star {
                    find_single_star(content)
                } else {
                    content.find(marker)
                };
                match close {
                    Some(rel) if !content[..rel].is_empty() => {
                        flush_plain(&mut spans, i, &mut plain_start);
                        spans.push(InlineSpan {
                            start: base + i + open_len,
                            end: base + i + open_len + rel,
                            kind,
                        });
                        i += open_len + rel + marker.len();
                        plain_start = i;
                    }
                    _ => {
                        // Unmatched marker: literal, advance one char.
                        i += text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                    }
                }
            }
            None => {
                i += text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            }
        }
    }
    flush_plain(&mut spans, text.len(), &mut plain_start);
    spans
}

/// Byte index of a `*` closer (one whose next char isn't `*`), if any.
fn find_single_star(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'*' {
            if bytes.get(i + 1) != Some(&b'*') {
                return Some(i);
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    None
}

/// Wrap visible text into byte ranges fitting `measure`, resolving each
/// candidate range's width through `width_of` (styling-aware: the caller
/// measures subranges in their own styles). Prefers breaking before spaces.
pub fn wrap_visible(
    visible: &str,
    measure: f32,
    width_of: &mut dyn FnMut(usize, usize) -> f32,
) -> Vec<(usize, usize)> {    if visible.is_empty() {
        return vec![(0, 0)];
    }
    if width_of(0, visible.len()) <= measure {
        return vec![(0, visible.len())];
    }
    // Words as (start, end) visible byte ranges (non-space runs).
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut wstart: Option<usize> = None;
    for (b, c) in visible.char_indices() {
        if c == ' ' || c == '\t' {
            if let Some(s) = wstart.take() {
                words.push((s, b));
            }
        } else if wstart.is_none() {
            wstart = Some(b);
        }
    }
    if let Some(s) = wstart {
        words.push((s, visible.len()));
    }
    if words.is_empty() {
        words.push((0, visible.len()));
    }
    let mut rows = Vec::new();
    let mut wi = 0;
    let mut row_start = 0;
    let mut row_has_word = false;
    while wi < words.len() {
        let (ws, we) = words[wi];
        // Width if the row ran to the word end (from row start).
        if width_of(row_start, we) <= measure {
            wi += 1;
            row_has_word = true;
            if wi == words.len() {
                rows.push((row_start, visible.len()));
            }
            continue;
        }
        if !row_has_word {
            // Single word overflows: force a mid-word break.
            let mut lo = row_start;
            let mut hi = we;
            while lo < hi {
                let mut mid = (lo + hi + 1) / 2;
                while mid > lo && !visible.is_char_boundary(mid) {
                    mid -= 1;
                }
                if mid <= lo {
                    break;
                }
                if width_of(row_start, mid) <= measure {
                    lo = mid;
                } else {
                    hi = mid.saturating_sub(1);
                }
            }
            if lo <= row_start {
                // Even one char overflows: force a single char of progress.
                lo = visible[row_start..]
                    .char_indices()
                    .nth(1)
                    .map(|(b, _)| row_start + b)
                    .unwrap_or(visible.len());
            }
            rows.push((row_start, lo));
            row_start = lo;
            // Same word continues on the next row.
            continue;
        }
        // Break before this word, skipping one space.
        let break_end = if ws > row_start && visible.as_bytes().get(ws - 1) == Some(&b' ') {
            ws - 1
        } else {
            ws
        };
        rows.push((row_start, break_end.max(row_start)));
        row_start = ws;
        row_has_word = false;
    }
    rows
}

/// Legacy source-row mapping used by cursor navigation. Rendering uses the
/// styled layout path in the shell, but navigation still needs raw source
/// byte ranges (including markdown markers).
#[derive(Clone, Debug)]
pub struct VisualRow {
    pub buffer_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

pub fn wrap_line(
    buffer_line: usize,
    text: &str,
    measure: f32,
    width_of: &mut dyn FnMut(&str) -> f32,
) -> Vec<VisualRow> {
    wrap_visible(text, measure, &mut |a, b| width_of(&text[a..b]))
        .into_iter()
        .map(|(start_byte, end_byte)| VisualRow { buffer_line, start_byte, end_byte })
        .collect()
}

pub fn byte_for_x(text: &str, x: f32, width_of: &mut dyn FnMut(&str) -> f32) -> usize {
    if x <= 0.0 {
        return 0;
    }
    let mut best = 0;
    for (b, _) in text.char_indices().skip(1) {
        let w = width_of(&text[..b]);
        if w > x {
            break;
        }
        best = b;
    }
    if width_of(text) <= x { text.len() } else { best }
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

    /// Replace the whole buffer (file open). Clears history/selection.
    pub fn set_text(&mut self, content: &str) {
        let mut rope = Rope::new();
        rope.insert(0, content);
        self.text = rope;
        self.cursor = 0;
        self.sel_anchor = None;
        self.undo.clear();
        self.redo.clear();
    }

    /// Whole buffer as a string (file save).
    pub fn text_string(&self) -> String {
        self.text.to_string()
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
        let text = "aa bb cc";
        let rows = wrap_visible(text, 45.0, &mut |a, b| (b - a) as f32 * 10.0);
        assert_eq!(rows, vec![(0, 2), (3, 5), (6, 8)]);
    }

    #[test]
    fn wrap_breaks_mid_word_without_spaces() {
        let text = "abcdefgh";
        let rows = wrap_visible(text, 35.0, &mut |a, b| (b - a) as f32 * 10.0);
        assert!(rows.len() >= 2);
        assert_eq!(rows[0].0, 0);
        assert_eq!(rows.last().unwrap().1, 8);
        for pair in rows.windows(2) {
            assert!(pair[1].0 >= pair[0].1);
        }
    }

    #[test]
    fn wrap_empty_is_one_row() {
        let rows = wrap_visible("", 100.0, &mut |_, _| 0.0);
        assert_eq!(rows, vec![(0, 0)]);
    }

    #[test]
    fn parse_hides_bold_markers() {
        let spans = parse_line("a **bold** c", true, 0);
        assert_eq!(spans.len(), 3);
        assert_eq!((spans[1].start, spans[1].end, spans[1].kind), (4, 8, SpanKind::Bold));
        // Visible text skips the markers.
        let visible: String = spans.iter().map(|s| &"a **bold** c"[s.start..s.end]).collect();
        assert_eq!(visible, "a bold c");
    }

    #[test]
    fn parse_shows_raw_when_active() {
        let line = "a **bold** c";
        let spans = parse_line(line, false, 0);
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].start, spans[0].end), (0, line.len()));
    }

    #[test]
    fn parse_unmatched_markers_stay_literal() {
        let spans = parse_line("a **oops", true, 0);
        assert!(spans.iter().all(|s| s.kind == SpanKind::Plain));
        let visible: String = spans.iter().map(|s| &"a **oops"[s.start..s.end]).collect();
        assert_eq!(visible, "a **oops");
    }

    #[test]
    fn parse_code_italic_strike() {
        let line = "`c` *i* ~~s~~";
        let kinds: Vec<SpanKind> =
            parse_line(line, true, 0).iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&SpanKind::Code));
        assert!(kinds.contains(&SpanKind::Italic));
        assert!(kinds.contains(&SpanKind::Strike));
    }

    #[test]
    fn hr_detection() {
        assert!(is_hr_line("---"));
        assert!(is_hr_line("  ***  "));
        assert!(!is_hr_line("--"));
        assert!(is_hr_line("___"));
        assert!(!is_hr_line("- - -"));
    }

    #[test]
    fn headings_detect_atx_levels() {
        assert_eq!(heading_level("plain"), 0);
        assert_eq!(heading_level("# Title"), 1);
        assert_eq!(heading_level("## Section"), 2);
        assert_eq!(heading_level("### Deep"), 3);
        assert_eq!(heading_level("###### Too deep"), 3);
        assert_eq!(heading_level("#nospace"), 0);
        assert_eq!(heading_level("#"), 1);
        assert_eq!(heading_level("  # indented"), 0);
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
