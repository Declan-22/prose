//! Root view: top bar, middle row (left dock + tab bar/manuscript +
//! right dock), bottom bar, menu overlay. Owns tabs, dock layout, menus,
//! hover tweens, and drag-resize state.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use gpui::{
    App, ClipboardItem, Context, FocusHandle, Font, FontFeatures, FontStyle, FontWeight, Hsla,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Point, Render, ScrollDelta,
    ScrollWheelEvent, ShapedLine, TextRun, WeakEntity, Window, div, prelude::*, px, rgba, svg,
};

use super::chrome::{BOTTOM_BAR_HEIGHT, BottomBar, TAB_BAR_HEIGHT, TOP_BAR_HEIGHT, TabBar, TopBar};
use super::dock::{DOCK_DEFAULT_WIDTH, DockView};
use super::dropdown::AppMenu;
use super::panels::{DockSide, PanelKind};
use crate::editor::EditorState;
use crate::persist::Store;
use crate::theme::WEIGHT_MEDIUM;

/// Plus-in-document icon for New File.
const ICON_NEW_FILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5">
		<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
		<path d="M14 2v6h6" />
		<path d="M12 18v-6" />
		<path d="M9 15h6" />
	</g>
</svg>"#;

/// Folder icon for Open File.
const ICON_OPEN_FILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5">
		<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
	</g>
</svg>"#;

#[derive(Clone)]
pub struct Tab {
    pub id: usize,
    pub title: String,
}

/// Layer toggle flags for the bottom-bar cluster.
pub const LAYER_SYNTAX: u8 = 0;
pub const LAYER_PROSE: u8 = 1;

const MANUSCRIPT_FONT_SIZE: f32 = 15.0;
const MANUSCRIPT_LINE_HEIGHT: f32 = 24.0;
const MANUSCRIPT_TOP_PAD: f32 = 24.0;
const RULER_HEIGHT: f32 = 30.0;
const CARET_HEIGHT: f32 = 17.0;
/// Logical pixels per ruler inch.
const PX_PER_INCH: f32 = 96.0;
/// Default manuscript measure (text width). Fixed once set — dock and
/// window resizes never re-wrap the text.
const DEFAULT_MEASURE: f32 = 720.0;
const MIN_MEASURE: f32 = 80.0;

/// Which ruler stick is being dragged.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MarginSide {
    Left,
    Right,
}

/// Resolved manuscript layout in center-column coordinates. Margins are
/// always identical, so the text is always centered in the column.
struct ManuscriptGeometry {
    /// Center-column width (window minus open docks).
    col_w: f32,
    /// Effective margin on both sides. Derived, never stored.
    margin: f32,
    /// Text width; lines wrap to this. Fixed unless the sticks move.
    measure: f32,
}

/// The measure is fixed: column changes eat the mirrored margins first.
/// At zero margins the text simply overflows and clips symmetrically —
/// it never re-wraps, and the column never auto-resizes.
fn manuscript_geometry(
    view_w: f32,
    left_occ: f32,
    right_occ: f32,
    measure_base: f32,
) -> ManuscriptGeometry {
    let col = (view_w - left_occ - right_occ).max(0.0);
    let margin = ((col - measure_base) / 2.0).max(0.0);
    ManuscriptGeometry { col_w: col, margin, measure: measure_base }
}

/// Render-ready snapshot of the active buffer. Keeps borrows short: the
/// entity builds this once, then renders from plain data.
struct SnapshotRow {
    text: String,
    /// Caret x (px) within the row, if the cursor is here.
    caret_x: Option<f32>,
    /// Selection x-range (px) within the row, if any overlaps it.
    sel_px: Option<(f32, f32)>,
}

struct EditorSnapshot {
    rows: Vec<SnapshotRow>,
    can_undo: bool,
    can_redo: bool,
    words: usize,
    has_selection: bool,
}

/// Shape-measured width of a manuscript string via the window text system.
fn measure_text(window: &mut Window, text: &str, color: Hsla) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    shape_row(window, text, color).width().as_f32()
}

/// Shape one manuscript row in the manuscript font. Callers split edges
/// from the returned layout so positions match rendered text exactly.
fn shape_row(window: &mut Window, text: &str, color: Hsla) -> ShapedLine {
    let font = Font {
        family: crate::theme::FONT_PROSE.into(),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    };
    let run = TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.to_string().into(), px(MANUSCRIPT_FONT_SIZE), &[run], None)
}

pub struct RootView {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_id: usize,
    /// Explicit dock assignment per panel; defaults live in `default_dock`.
    panel_dock: HashMap<PanelKind, DockSide>,
    visible_left: Option<PanelKind>,
    visible_right: Option<PanelKind>,
    left_width: f32,
    right_width: f32,
    /// Active dock drag: (side being resized, last cursor x, window width).
    /// Widths are unclamped; the only rule is the docks never overlap.
    drag: Option<(DockSide, f32, f32)>,
    /// Panel whose Dock Left / Dock Right popup is open, if any.
    dock_menu: Option<PanelKind>,
    preview_style: Option<String>,
    syntax_on: bool,
    prose_on: bool,
    recent_files: Vec<String>,
    hovered_tab: Option<usize>,
    store: Store,
    /// Buffers keyed by tab id, created lazily.
    editors: HashMap<usize, EditorState>,
    /// Manuscript focus follows the active tab.
    pub focus: FocusHandle,
    /// Manuscript measure base (text width). The only ruler quantity
    /// that is stored; margins always derive from it.
    measure_base: f32,
    /// Active ruler-stick drag: (side, start x, start measure).
    margin_drag: Option<(MarginSide, f32, f32)>,
    /// Caret blink state, toggled on a timer.
    caret_visible: bool,
    blink_started: bool,
    /// Vertical scroll offset (px) of the manuscript viewport.
    scroll_offset: f32,
    /// True while the left button is held for manuscript mouse selection.
    mouse_select: bool,
    // -- hamburger menu state (Parametric behavior) --
    pub menu_open: bool,
    pub active_menu: Option<usize>,
    pub menu_animation: f32,
    pub icon_animation: f32,
    pub hovered_entry: Option<usize>,
    fades: HashMap<String, f32>,
    fade_pending: HashMap<String, f32>,
    fade_tween_active: HashSet<String>,
    cursor_trail: Vec<(Point<gpui::Pixels>, Instant)>,
}

impl RootView {
    pub fn new(cx: &mut Context<Self>, store: Store) -> Self {
        // Restore tabs; fall back to a fresh Welcome + Untitled session.
        let mut tabs: Vec<Tab> = Vec::new();
        let mut next_tab_id = 0;
        if let Some(saved) = store.get("tabs.titles") {
            for title in saved.split('\n').filter(|s| !s.is_empty()) {
                tabs.push(Tab { id: next_tab_id, title: title.to_string() });
                next_tab_id += 1;
            }
        }
        if tabs.is_empty() {
            tabs.push(Tab { id: 0, title: "Welcome".into() });
            tabs.push(Tab { id: 1, title: "Untitled".into() });
            next_tab_id = 2;
        }
        let active_tab = store
            .get("tabs.active")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(tabs.len() - 1)
            .min(tabs.len() - 1);

        let width = |key: &str, fallback: f32| {
            store.get(key).and_then(|v| v.parse::<f32>().ok()).unwrap_or(fallback)
        };
        let visible = |key: &str| store.get(key).and_then(|v| PanelKind::from_key(&v));
        let mut panel_dock = HashMap::new();
        for kind in PanelKind::all() {
            if let Some(side) = store
                .get(&format!("panels.dock.{}", kind.key()))
                .map(|v| if v == "left" { DockSide::Left } else { DockSide::Right })
            {
                panel_dock.insert(kind, side);
            }
        }

        let base_measure = width("manuscript.measure", DEFAULT_MEASURE);

        Self {
            tabs,
            active_tab,
            next_tab_id,
            panel_dock,
            visible_left: visible("panels.left.visible"),
            visible_right: visible("panels.right.visible"),
            left_width: width("panels.left.width", DOCK_DEFAULT_WIDTH),
            right_width: width("panels.right.width", DOCK_DEFAULT_WIDTH),
            drag: None,
            dock_menu: None,
            preview_style: None,
            syntax_on: false,
            prose_on: false,
            recent_files: Vec::new(),
            hovered_tab: None,
            store,
            editors: HashMap::new(),
            focus: cx.focus_handle(),
            measure_base: base_measure,
            margin_drag: None,
            caret_visible: true,
            blink_started: false,
            scroll_offset: 0.0,
            mouse_select: false,
            menu_open: false,
            active_menu: None,
            menu_animation: 0.0,
            icon_animation: 0.0,
            hovered_entry: None,
            fades: HashMap::new(),
            fade_pending: HashMap::new(),
            fade_tween_active: HashSet::new(),
            cursor_trail: Vec::new(),
        }
    }

    fn default_dock(kind: PanelKind) -> DockSide {
        match kind {
            PanelKind::Project | PanelKind::Outline => DockSide::Left,
            PanelKind::Styling
            | PanelKind::Inspector
            | PanelKind::History
            | PanelKind::Comments => DockSide::Right,
        }
    }

    pub fn dock_of(&self, kind: PanelKind) -> DockSide {
        self.panel_dock.get(&kind).copied().unwrap_or(Self::default_dock(kind))
    }

    fn panels_on(&self, side: DockSide) -> Vec<PanelKind> {
        PanelKind::all().into_iter().filter(|k| self.dock_of(*k) == side).collect()
    }

    fn save_layout(&self) {
        let opt = |v: Option<PanelKind>| v.map(|k| k.key()).unwrap_or("");
        self.store.set("panels.left.visible", opt(self.visible_left));
        self.store.set("panels.right.visible", opt(self.visible_right));
        self.store.set("panels.left.width", &self.left_width.to_string());
        self.store.set("panels.right.width", &self.right_width.to_string());
        for kind in PanelKind::all() {
            let side = if self.dock_of(kind) == DockSide::Left {
                "left"
            } else {
                "right"
            };
            self.store
                .set(&format!("panels.dock.{}", kind.key()), side);
        }
        let titles: Vec<&str> = self.tabs.iter().map(|t| t.title.as_str()).collect();
        self.store.set("tabs.titles", &titles.join("\n"));
        self.store.set("tabs.active", &self.active_tab.to_string());
        self.store.set("manuscript.measure", &self.measure_base.to_string());
    }

    // ----- tabs -----

    pub fn activate_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
            self.active_tab = i;
            self.save_layout();
            cx.notify();
        }
    }

    pub fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(i);
            if self.tabs.is_empty() {
                self.tabs.push(Tab { id: self.next_tab_id, title: "Welcome".into() });
                self.next_tab_id += 1;
            }
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
            self.save_layout();
            cx.notify();
        }
    }

    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let id = tab.id;
            self.close_tab(id, cx);
        }
    }

    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab { id, title: format!("Untitled {}", id) });
        self.active_tab = self.tabs.len() - 1;
        self.save_layout();
        cx.notify();
    }

    pub fn open_welcome(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.tabs.iter().position(|t| t.title == "Welcome") {
            self.active_tab = i;
        } else {
            let id = self.next_tab_id;
            self.next_tab_id += 1;
            self.tabs.push(Tab { id, title: "Welcome".into() });
            self.active_tab = self.tabs.len() - 1;
        }
        self.save_layout();
        cx.notify();
    }

    // ----- manuscript geometry + ruler -----

    fn dock_occupancy(&self) -> (f32, f32) {
        (
            self.visible_left.map(|_| self.left_width).unwrap_or(0.0),
            self.visible_right.map(|_| self.right_width).unwrap_or(0.0),
        )
    }

    fn geometry(&self, window: &mut Window) -> ManuscriptGeometry {
        let view_w = window.viewport_size().width.as_f32();
        let (left_occ, right_occ) = self.dock_occupancy();
        manuscript_geometry(view_w, left_occ, right_occ, self.measure_base)
    }

    pub fn begin_margin_drag(&mut self, side: MarginSide, x: f32, cx: &mut Context<Self>) {
        self.margin_drag = Some((side, x, self.measure_base));
        cx.notify();
    }

    /// Either stick sets the measure: dragging the left stick right or the
    /// right stick left narrows the text, mirrored. Margins re-derive.
    fn margin_drag_to(&mut self, x: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some((side, start_x, start_measure)) = self.margin_drag else {
            return;
        };
        let geo = self.geometry(window);
        let dx = x - start_x;
        let delta = match side {
            MarginSide::Left => dx * 2.0,
            MarginSide::Right => -dx * 2.0,
        };
        let max = geo.col_w.max(MIN_MEASURE);
        let next = (start_measure + delta).clamp(MIN_MEASURE, max);
        self.measure_base = next;
        self.margin_drag = Some((side, x, next));
        cx.notify();
    }

    // ----- undo / redo (active tab's buffer) -----

    fn active_editor_mut(&mut self) -> Option<&mut EditorState> {
        let tab = self.tabs.get(self.active_tab)?;
        if tab.title == "Welcome" {
            return None;
        }
        Some(self.editors.entry(tab.id).or_insert_with(EditorState::new))
    }

    fn handle_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let mods = &ev.keystroke.modifiers;
        // Clipboard + select-all chords bypass the buffer entirely.
        if (mods.control || mods.platform) && !mods.alt {
            match ev.keystroke.key.as_str() {
                "c" => {
                    if let Some(ed) = self.active_editor_mut() {
                        if let Some(text) = ed.selected_text() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                        }
                    }
                    return;
                }
                "x" => {
                    if let Some(ed) = self.active_editor_mut() {
                        if let Some(text) = ed.selected_text() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            ed.delete_selection();
                            self.caret_visible = true;
                            cx.notify();
                        }
                    }
                    return;
                }
                "v" => {
                    if let Some(ed) = self.active_editor_mut() {
                        if let Some(item) = cx.read_from_clipboard() {
                            if let Some(text) = item.text() {
                                ed.delete_selection();
                                ed.insert_str(&text);
                                self.caret_visible = true;
                                cx.notify();
                            }
                        }
                    }
                    return;
                }
                "a" => {
                    if let Some(ed) = self.active_editor_mut() {
                        ed.select_all();
                        cx.notify();
                    }
                    return;
                }
                _ => return,
            }
        }
        let Some(ed) = self.active_editor_mut() else {
            return;
        };
        let key = ev.keystroke.key.as_str();
        let clean = !mods.control && !mods.platform && !mods.alt;
        let extend = mods.shift;
        match key {
            "left" if clean => ed.move_left(extend),
            "right" if clean => ed.move_right(extend),
            "home" if clean => ed.line_home(extend),
            "end" if clean => ed.line_end(extend),
            "up" if clean => self.move_visual(-1, extend, window, cx),
            "down" if clean => self.move_visual(1, extend, window, cx),
            "backspace" if clean => ed.backspace(),
            "delete" if clean => ed.delete_forward(),
            "enter" if clean => ed.insert_str("\n"),
            "tab" => return,
            _ => {
                if !clean {
                    return;
                }
                match &ev.keystroke.key_char {
                    Some(s) => {
                        let filtered: String = s.chars().filter(|c| *c != '\r').collect();
                        if filtered.is_empty() {
                            return;
                        }
                        ed.insert_str(&filtered);
                    }
                    None => return,
                }
            }
        }
        self.caret_visible = true;
        cx.notify();
    }

    /// Vertical movement by visual (wrapped) row, preserving pixel x.
    fn move_visual(&mut self, dir: i32, extend: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_id) = self
            .tabs
            .get(self.active_tab)
            .filter(|tab| tab.title != "Welcome")
            .map(|tab| tab.id)
        else {
            return;
        };
        // Measure with the window text system (cached per content).
        let color = Hsla::from(rgba(crate::theme::active(cx).text_primary));
        let geo = self.geometry(window);
        let mut width_of = |s: &str| -> f32 {
            if s.is_empty() {
                return 0.0;
            }
            let font = Font {
                family: crate::theme::FONT_PROSE.into(),
                features: FontFeatures::default(),
                fallbacks: None,
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
            };
            let run = TextRun {
                len: s.len(),
                font,
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_line(s.to_string().into(), px(MANUSCRIPT_FONT_SIZE), &[run], None)
                .width()
                .as_f32()
        };
        let ed = self.editors.entry(tab_id).or_insert_with(EditorState::new);
        let (cur_line, cur_byte) = ed.cursor_line_byte();
        // Rows for the cursor line and its neighbors.
        let mut rows = Vec::new();
        for line in cur_line.saturating_sub(1)..=(cur_line + 1).min(ed.len_lines().saturating_sub(1)) {
            let text = ed.line_text(line);
            rows.extend(crate::editor::wrap_line(line, &text, geo.measure, &mut width_of));
        }
        let mut current = None;
        for (i, row) in rows.iter().enumerate() {
            if row.buffer_line == cur_line && row.start_byte <= cur_byte && cur_byte <= row.end_byte
            {
                current = Some(i);
                break;
            }
        }
        let Some(current) = current else { return };
        let target = current as i32 + dir;
        if target < 0 || target >= rows.len() as i32 {
            return;
        }
        let target = &rows[target as usize];
        // Caret x on the current row, then the widest prefix fitting it.
        let cur_row = &rows[current];
        let rel = cur_byte.min(cur_row.end_byte).saturating_sub(cur_row.start_byte);
        let row_text = ed.line_text(cur_row.buffer_line);
        let row_slice = &row_text[cur_row.start_byte..cur_row.end_byte.min(row_text.len())];
        let prefix = &row_slice[..rel.min(row_slice.len())];
        let x = width_of(prefix);
        let target_text = ed.line_text(target.buffer_line);
        let target_slice =
            &target_text[target.start_byte..target.end_byte.min(target_text.len())];
        let lo = crate::editor::byte_for_x(target_slice, x, &mut width_of);
        ed.set_cursor_line_byte(target.buffer_line, target.start_byte + lo, extend);
    }

    /// Wheel scrolling over the manuscript viewport.
    fn handle_wheel(&mut self, ev: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let dy = match ev.delta {
            ScrollDelta::Pixels(p) => p.y.as_f32(),
            ScrollDelta::Lines(p) => p.y * MANUSCRIPT_LINE_HEIGHT,
        };
        if dy == 0.0 {
            return;
        }
        let color = Hsla::from(rgba(crate::theme::active(cx).text_primary));
        let max = self.max_scroll(window, color);
        self.scroll_offset = (self.scroll_offset + dy).clamp(0.0, max);
        cx.notify();
    }

    /// Maximum scroll offset for the active buffer at the current wrap.
    fn max_scroll(&mut self, window: &mut Window, color: Hsla) -> f32 {
        let Some(tab_id) = self
            .tabs
            .get(self.active_tab)
            .filter(|tab| tab.title != "Welcome")
            .map(|tab| tab.id)
        else {
            return 0.0;
        };
        let geo = self.geometry(window);
        let view_h = Self::viewport_h(window);
        let mut width_of = |s: &str| -> f32 { measure_text(window, s, color) };
        let ed = self.editors.entry(tab_id).or_insert_with(EditorState::new);
        let mut count = 0;
        for line_no in 0..ed.len_lines() {
            let text = ed.line_text(line_no);
            count += crate::editor::wrap_line(line_no, &text, geo.measure, &mut width_of).len();
        }
        ((count as f32 * MANUSCRIPT_LINE_HEIGHT) - view_h).max(0.0)
    }

    /// Move the cursor to a window point (click / drag). Coordinates are
    /// mapped through scroll offset, top padding, margins, and the wrap
    /// map, so the cursor lands on the exact character under the pointer.
    fn position_from_point(
        &mut self,
        x_win: f32,
        y_win: f32,
        window: &mut Window,
        extend: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self
            .tabs
            .get(self.active_tab)
            .filter(|tab| tab.title != "Welcome")
            .map(|tab| tab.id)
        else {
            return;
        };
        let color = Hsla::from(rgba(crate::theme::active(cx).text_primary));
        let geo = self.geometry(window);
        let (left_occ, _) = self.dock_occupancy();
        let x_col = x_win - left_occ;
        // Ignore clicks landing on the scrollbar gutter.
        if x_col > geo.col_w - 18.0 {
            return;
        }
        let y_view = y_win - (TOP_BAR_HEIGHT + TAB_BAR_HEIGHT + RULER_HEIGHT);
        let view_h = Self::viewport_h(window);
        let mut width_of = |s: &str| -> f32 { measure_text(window, s, color) };
        let ed = self.editors.entry(tab_id).or_insert_with(EditorState::new);
        let mut rows = Vec::new();
        for line_no in 0..ed.len_lines() {
            let text = ed.line_text(line_no);
            rows.extend(crate::editor::wrap_line(line_no, &text, geo.measure, &mut width_of));
        }
        if rows.is_empty() {
            return;
        }
        let total_h = rows.len() as f32 * MANUSCRIPT_LINE_HEIGHT;
        let max = (total_h - view_h).max(0.0);
        let offset = self.scroll_offset.min(max);
        let content_y = y_view + offset;
        let idx = ((content_y - MANUSCRIPT_TOP_PAD) / MANUSCRIPT_LINE_HEIGHT)
            .floor()
            .max(0.0) as usize;
        let idx = idx.min(rows.len() - 1);
        let row = &rows[idx];
        // Text block is centered: its left edge in column coordinates.
        let text_left = (geo.col_w - geo.measure) / 2.0;
        let x_text = x_col - text_left;
        let row_text = ed.line_text(row.buffer_line);
        let row_slice = &row_text[row.start_byte..row.end_byte.min(row_text.len())];
        let byte = crate::editor::byte_for_x(row_slice, x_text, &mut width_of);
        ed.set_cursor_line_byte(row.buffer_line, row.start_byte + byte, extend);
        self.caret_visible = true;
        cx.notify();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(ed) = self.active_editor_mut() {
            ed.undo();
        }
        cx.notify();
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(ed) = self.active_editor_mut() {
            ed.redo();
        }
        cx.notify();
    }

    // ----- hamburger menu state -----

    pub fn record_cursor(&mut self, pos: Point<gpui::Pixels>) {
        let now = Instant::now();
        self.cursor_trail.push((pos, now));
        self.cursor_trail
            .retain(|(_, t)| now.duration_since(*t).as_millis() <= 120);
    }

    // Velocity in px over the recent trail.
    pub fn cursor_velocity(&self) -> (f32, f32) {
        let Some((first, t0)) = self.cursor_trail.first() else {
            return (0., 0.);
        };
        let Some((last, t1)) = self.cursor_trail.last() else {
            return (0., 0.);
        };
        let dt = t1.duration_since(*t0).as_secs_f32();
        if dt < f32::EPSILON {
            return (0., 0.);
        }
        (
            (last.x - first.x).as_f32() / dt,
            (last.y - first.y).as_f32() / dt,
        )
    }

    pub fn toggle_menu(&mut self, cx: &mut Context<Self>) {
        self.menu_open = !self.menu_open;
        // Every transition — open or close — starts from cold state. Hover
        // looks must come from live hover events only, never leftovers.
        self.reset_menu_state();
        self.menu_animation = 0.0;
        self.animate_icon(self.menu_open, cx);
        if self.menu_open {
            self.start_menu_animation(cx);
        }
        cx.notify();
    }

    /// Clear submenu, hover tracking, and hover tweens.
    fn reset_menu_state(&mut self) {
        self.active_menu = None;
        self.hovered_entry = None;
        self.fades.retain(|k, _| {
            !k.starts_with("menu-entry-") && !k.starts_with("submenu-")
        });
        self.fade_pending
            .retain(|k, _| !k.starts_with("menu-entry-") && !k.starts_with("submenu-"));
        self.fade_tween_active
            .retain(|k| !k.starts_with("menu-entry-") && !k.starts_with("submenu-"));
    }

    // Fades the menu icon between its idle and active styling.
    pub fn animate_icon(&mut self, opening: bool, cx: &mut Context<Self>) {
        let start = self.icon_animation;
        let end = if opening { 1.0 } else { 0.0 };
        cx.spawn(async move |this, cx| {
            let steps = 6;
            for i in 1..=steps {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let _ = this.update(cx, |root, cx| {
                    root.icon_animation = start + (end - start) * (i as f32 / steps as f32);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn start_menu_animation(&mut self, cx: &mut Context<Self>) {
        self.menu_animation = 0.0;
        cx.spawn(async move |this, cx| {
            let steps = 6;
            for i in 1..=steps {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let _ = this.update(cx, |root, cx| {
                    root.menu_animation = i as f32 / steps as f32;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        self.reset_menu_state();
        self.menu_animation = 0.0;
        self.animate_icon(false, cx);
        cx.notify();
    }

    pub fn set_active_menu(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        if self.active_menu == index {
            return;
        }
        self.active_menu = index;
        cx.notify();
    }

    // ----- hover fades -----

    pub fn fade(&self, key: &str) -> f32 {
        self.fades.get(key).copied().unwrap_or(0.0)
    }

    // Tweens a named fade toward the target (0 or 1). A single ticker per
    // key eases exponentially toward whatever the pending target is, so
    // rapid hover in/out never fights itself.
    pub fn animate_fade(&mut self, key: &str, target: f32, cx: &mut Context<Self>) {
        self.fade_pending.insert(key.to_string(), target);
        if !self.fade_tween_active.insert(key.to_string()) {
            return;
        }
        let key_owned = key.to_string();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(12))
                    .await;
                let mut done = false;
                let _ = this.update(cx, |root, cx| {
                    if let Some(&target) = root.fade_pending.get(&key_owned) {
                        let cur = root.fade(&key_owned);
                        let next = cur + (target - cur) * 0.4;
                        if (next - target).abs() < 0.01 {
                            root.fades.insert(key_owned.clone(), target);
                            done = true;
                        } else {
                            root.fades.insert(key_owned.clone(), next);
                        }
                        cx.notify();
                    } else {
                        done = true;
                    }
                });
                if !done {
                    continue;
                }
                let _ = this.update(cx, |root, _| {
                    root.fade_pending.remove(&key_owned);
                    root.fade_tween_active.remove(&key_owned);
                });
                break;
            }
        })
        .detach();
    }

    pub fn set_hovered_tab(&mut self, id: Option<usize>, cx: &mut Context<Self>) {
        if self.hovered_tab != id {
            self.hovered_tab = id;
            cx.notify();
        }
    }

    // ----- panels -----

    pub fn toggle_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        self.dock_menu = None;
        match self.dock_of(kind) {
            DockSide::Left => {
                self.visible_left = if self.visible_left == Some(kind) { None } else { Some(kind) };
            }
            DockSide::Right => {
                self.visible_right =
                    if self.visible_right == Some(kind) { None } else { Some(kind) };
            }
        }
        self.save_layout();
        cx.notify();
    }

    pub fn set_dock(&mut self, kind: PanelKind, side: DockSide, cx: &mut Context<Self>) {
        self.panel_dock.insert(kind, side);
        match side {
            DockSide::Left => {
                self.visible_left = Some(kind);
                if self.visible_right == Some(kind) {
                    self.visible_right = None;
                }
            }
            DockSide::Right => {
                self.visible_right = Some(kind);
                if self.visible_left == Some(kind) {
                    self.visible_left = None;
                }
            }
        }
        self.save_layout();
        cx.notify();
    }

    pub fn open_dock_menu(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        self.dock_menu = Some(kind);
        cx.notify();
    }

    pub fn close_dock_menu(&mut self, cx: &mut Context<Self>) {
        if self.dock_menu.take().is_some() {
            cx.notify();
        }
    }

    pub fn toggle_preview_style(&mut self, name: &str, cx: &mut Context<Self>) {
        self.preview_style = if self.preview_style.as_deref() == Some(name) {
            None
        } else {
            Some(name.to_string())
        };
        cx.notify();
    }

    pub fn toggle_layer(&mut self, flag: u8, cx: &mut Context<Self>) {
        match flag {
            LAYER_SYNTAX => self.syntax_on = !self.syntax_on,
            LAYER_PROSE => self.prose_on = !self.prose_on,
            _ => {}
        }
        cx.notify();
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        crate::theme::toggle(cx);
        let mode = match crate::theme::mode(cx) {
            crate::theme::ThemeMode::Light => "light",
            crate::theme::ThemeMode::Dark => "dark",
        };
        self.store.set("theme.mode", mode);
        cx.notify();
    }

    // ----- files -----

    /// Native open-file dialog. Blocks the UI thread while open, like any
    /// native modal; the pick itself is instant.
    pub fn pick_file(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.open_file_path(path.to_string_lossy().into_owned(), cx);
        }
    }

    pub fn open_file_path(&mut self, path: String, cx: &mut Context<Self>) {
        let title = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or(path.clone());
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(12);
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab { id, title });
        self.active_tab = self.tabs.len() - 1;
        self.save_layout();
        cx.notify();
    }

    // ----- dock drag-resize (unclamped; docks just can't overlap) -----

    pub fn begin_dock_drag(&mut self, side: DockSide, x: f32, max_total: f32, cx: &mut Context<Self>) {
        self.drag = Some((side, x, max_total));
        cx.notify();
    }

    fn drag_to(&mut self, x: f32, cx: &mut Context<Self>) {
        if let Some((side, last, max_total)) = self.drag {
            let dx = x - last;
            match side {
                DockSide::Left => {
                    let cap = (max_total - self.right_width).max(0.0);
                    self.left_width = (self.left_width + dx).clamp(0.0, cap);
                }
                DockSide::Right => {
                    let cap = (max_total - self.left_width).max(0.0);
                    self.right_width = (self.right_width - dx).clamp(0.0, cap);
                }
            }
            self.drag = Some((side, x, max_total));
            cx.notify();
        }
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        let was_dragging = self.drag.take().is_some() || self.margin_drag.take().is_some();
        self.mouse_select = false;
        if was_dragging {
            self.save_layout();
            cx.notify();
        }
    }

    /// Viewport height: window minus top bar, tab bar, ruler, bottom bar.
    fn viewport_h(window: &mut Window) -> f32 {
        (window.viewport_size().height.as_f32()
            - TOP_BAR_HEIGHT
            - TAB_BAR_HEIGHT
            - RULER_HEIGHT
            - BOTTOM_BAR_HEIGHT)
            .max(0.0)
    }

    // ----- render -----

    fn render_manuscript(
        &self,
        root: WeakEntity<RootView>,
        snapshot: Option<EditorSnapshot>,
        focus: FocusHandle,
        geo: &ManuscriptGeometry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let title = self.tabs.get(self.active_tab).map(|t| t.title.clone()).unwrap_or_default();
        if title == "Welcome" {
            return self.render_welcome(root, cx);
        }
        let t = *crate::theme::active(cx);
        let snap = snapshot.unwrap_or(EditorSnapshot {
            rows: vec![SnapshotRow { text: String::new(), caret_x: Some(0.0), sel_px: None }],
            can_undo: false,
            can_redo: false,
            words: 0,
            has_selection: false,
        });
        // No caret while any selection is visible.
        let show_caret = self.caret_visible && !snap.has_selection;

        // Scrollbar geometry (Zed-style: thick low-opacity thumb + a 2px
        // accent line at the cursor row).
        let view_h = Self::viewport_h(window);
        let total_h = snap.rows.len() as f32 * MANUSCRIPT_LINE_HEIGHT;
        let offset = self.scroll_offset.min((total_h - view_h).max(0.0));
        let show_scroll = total_h > view_h;
        let track_h = (view_h - 4.0).max(0.0);
        let thumb_h = if show_scroll {
            (view_h / total_h * track_h).max(24.0).min(track_h)
        } else {
            0.0
        };
        let thumb_top = if show_scroll {
            let max = (total_h - view_h).max(0.0);
            if max <= 0.0 {
                2.0
            } else {
                2.0 + (offset / max) * (track_h - thumb_h).max(0.0)
            }
        } else {
            0.0
        };
        let cursor_y = if show_scroll && total_h > 0.0 {
            let idx = snap.rows.iter().position(|r| r.caret_x.is_some()).unwrap_or(0);
            2.0 + ((idx as f32 * MANUSCRIPT_LINE_HEIGHT + MANUSCRIPT_LINE_HEIGHT / 2.0) / total_h)
                * track_h
        } else {
            0.0
        };

        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .items_center()
            .overflow_hidden()
            .bg(rgba(t.bg_primary))
            .cursor_text()
            .child(self.render_ruler(root.clone(), geo, t))
            .child(
                div()
                    .id("manuscript-viewport")
                    .relative()
                    .w_full()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .track_focus(&focus)
                    .on_mouse_down(MouseButton::Left, {
                        let root = root.clone();
                        let fh = focus.clone();
                        move |ev: &MouseDownEvent, window, cx| {
                            window.focus(&fh, cx);
                            let _ = root.update(cx, |root, cx| {
                                root.mouse_select = true;
                                root.position_from_point(
                                    ev.position.x.as_f32(),
                                    ev.position.y.as_f32(),
                                    window,
                                    ev.modifiers.shift,
                                    cx,
                                );
                            });
                        }
                    })
                    .on_mouse_move(cx.listener(
                        |this, ev: &gpui::MouseMoveEvent, window, cx| {
                            if this.mouse_select {
                                this.position_from_point(
                                    ev.position.x.as_f32(),
                                    ev.position.y.as_f32(),
                                    window,
                                    true,
                                    cx,
                                );
                            }
                        },
                    ))
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        this.handle_key(ev, window, cx);
                    }))
                    .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, window, cx| {
                        this.handle_wheel(ev, window, cx);
                    }))
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .right(px(0.))
                            .top(px(-offset))
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                div()
                                    .w(px(geo.measure))
                                    .flex()
                                    .flex_col()
                                    .pt(px(MANUSCRIPT_TOP_PAD))
                                    .children(snap.rows.iter().map(|row| {
                                        // The text always renders as one
                                        // unsplit run, so shaping (kerning,
                                        // ligatures) never shifts when the
                                        // selection or caret moves.
                                        // Selection and caret are overlays.
                                        div()
                                            .relative()
                                            .w_full()
                                            .h(px(MANUSCRIPT_LINE_HEIGHT))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .font_family(crate::theme::FONT_PROSE)
                                            .text_size(px(MANUSCRIPT_FONT_SIZE))
                                            .text_color(rgba(t.text_primary))
                                            .child(row.text.clone())
                                            .when_some(row.sel_px, |r, range| {
                                                let (x0, x1) = range;
                                                r.child(
                                                    div()
                                                        .absolute()
                                                        .left(px(x0))
                                                        .top(px(0.))
                                                        .w(px((x1 - x0).max(0.0)))
                                                        .h_full()
                                                        .bg(rgba(t.selection)),
                                                )
                                            })
                                            .when_some(
                                                row.caret_x.filter(|_| show_caret),
                                                |r, x| {
                                                    r.child(
                                                        div()
                                                            .absolute()
                                                            .left(px(x))
                                                            .top(px(
                                                                (MANUSCRIPT_LINE_HEIGHT
                                                                    - CARET_HEIGHT)
                                                                    / 2.0,
                                                            ))
                                                            .w(px(2.))
                                                            .h(px(CARET_HEIGHT))
                                                            .bg(rgba(t.cursor)),
                                                    )
                                                },
                                            )
                                    })
                                )
                            )
                    .when(show_scroll, |v| {
                        v.child(
                            div()
                                .absolute()
                                .right(px(2.))
                                .top(px(2.))
                                .bottom(px(2.))
                                .w(px(12.))
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(0.))
                                        .right(px(0.))
                                        .top(px(thumb_top - 2.))
                                        .h(px(thumb_h))
                                        .rounded(px(6.))
                                        .bg(rgba(t.text_secondary))
                                        .opacity(0.4),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(0.))
                                        .right(px(0.))
                                        .top(px(cursor_y - 1.))
                                        .h(px(2.))
                                        .bg(rgba(t.accent)),
                                )
                        )
                    })
                )
            )
            .into_any_element()
    }

    /// Docs-style margin ruler: inch numbers and ticks across the column,
    /// shaded outside-margin zones, and two blue margin markers that drag
    /// the (mirrored) wrap margins. Markers always sit on the wrap edges.
    fn render_ruler(
        &self,
        root: WeakEntity<RootView>,
        geo: &ManuscriptGeometry,
        t: crate::theme::Theme,
    ) -> gpui::AnyElement {
        let text_left = geo.margin;
        let text_right = text_left + geo.measure;
        let shade = rgba(t.bg_tertiary);
        let tick = rgba(t.line_number);
        let num = rgba(t.empty_text_primary);
        let marker = rgba(t.text_secondary);

        // Blue margin marker: a 2px rounded line, vertically centered,
        // inside a wider invisible grab handle.
        let marker_el = |side: MarginSide, x: f32| {
            let root = root.clone();
            div()
                .id(match side {
                    MarginSide::Left => "ruler-stick-left",
                    MarginSide::Right => "ruler-stick-right",
                })
                .absolute()
                .left(px(x - 6.))
                .top(px(0.))
                .bottom(px(0.))
                .w(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .cursor(gpui::CursorStyle::ResizeLeftRight)
                .on_mouse_down(MouseButton::Left, move |ev: &gpui::MouseDownEvent, _, cx| {
                    let _ = root.update(cx, |root, cx| {
                        root.begin_margin_drag(side, ev.position.x.as_f32(), cx);
                    });
                })
                .child(
                    div()
                        .w(px(2.))
                        .h(px(18.))
                        .rounded_full()
                        .bg(marker),
                )
        };

        // Inch numbers + ticks (96px per inch).
        let inches = (geo.col_w / PX_PER_INCH) as usize;
        let tick_count = (geo.col_w / 12.0) as usize;
        div()
            .relative()
            .w_full()
            .h(px(RULER_HEIGHT))
            .bg(rgba(t.bg_primary))
            .border_b_1()
            .border_color(rgba(t.chrome_border_color))
            .overflow_hidden()
            // Outside-margin shading.
            .child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .bottom(px(0.))
                    .w(px(text_left.max(0.)))
                    .bg(shade),
            )
            .child(
                div()
                    .absolute()
                    .left(px(text_right))
                    .top(px(0.))
                    .bottom(px(0.))
                    .right(px(0.))
                    .bg(shade),
            )
            // Ticks every 1/8", medium every 1/2", tall every inch.
            .children((0..=tick_count).map(|k| {
                let x = k as f32 * 12.0;
                let h = if x % PX_PER_INCH == 0.0 {
                    8.
                } else if x % 48.0 == 0.0 {
                    5.
                } else {
                    3.
                };
                div()
                    .absolute()
                    .left(px(x))
                    .bottom(px(0.))
                    .w(px(1.))
                    .h(px(h))
                    .bg(tick)
            }))
            // Inch numbers in mono.
            .children((1..=inches).map(|k| {
                let x = k as f32 * PX_PER_INCH;
                div()
                    .absolute()
                    .left(px(x - 16.))
                    .top(px(1.))
                    .w(px(32.))
                    .text_center()
                    .text_size(px(9.))
                    .font_family(crate::theme::FONT_MONO)
                    .text_color(num)
                    .child(k.to_string())
            }))
            .child(marker_el(MarginSide::Left, text_left))
            .child(marker_el(MarginSide::Right, text_right))
            .into_any_element()
    }

    /// Zed-style welcome tab: big centered greeting, then a narrow
    /// left-aligned column of Get Started actions and recent files.
    fn render_welcome(&self, root: WeakEntity<RootView>, cx: &mut App) -> gpui::AnyElement {
        let t = *crate::theme::active(cx);

        // Uppercase section label in mono, with a 1px rule taking the
        // remaining width, vertically centered with the text.
        let section = |label: &'static str| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .mb(px(8.))
                .child(
                    div()
                        .text_xs()
                        .font_family(crate::theme::FONT_MONO)
                        .text_color(rgba(t.text_secondary))
                        .child(label),
                )
                .child(div().flex_1().h(px(1.)).bg(rgba(t.chrome_border_color)))
        };

        let action_button = |id: &'static str, icon: &'static [u8], label: &'static str, cmd: u8| {
            let root = root.clone();
            div()
                .id(id)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .h(px(30.))
                .px(px(6.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let _ = root.update(cx, |root, cx| {
                        if cmd == 0 {
                            root.new_tab(cx);
                        } else {
                            root.pick_file(cx);
                        }
                    });
                })
                .child(
                    svg()
                        .data(icon)
                        .w(px(14.))
                        .h(px(14.))
                        .text_color(rgba(t.text_secondary)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(WEIGHT_MEDIUM)
                        .text_color(rgba(t.text_primary))
                        .child(label),
                )
        };

        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgba(t.bg_primary))
            .child(
                div()
                    .w_full()
                    .max_w(px(560.))
                    .px(px(48.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .text_2xl()
                            .font_weight(FontWeight(500.0))
                            .text_color(rgba(t.text_primary))
                            .mb(px(28.))
                            .child("Welcome back to Prose"),
                    )
                    .child(section("GET STARTED"))
                    .child(action_button("welcome-new", ICON_NEW_FILE, "New File", 0))
                    .child(action_button("welcome-open", ICON_OPEN_FILE, "Open File", 1))
                    .child(div().h(px(20.)))
                    .when(!self.recent_files.is_empty(), |col| {
                        col.child(section("RECENT PROJECTS")).children(
                            self.recent_files.iter().map(|path| {
                                let root = root.clone();
                                let path = path.clone();
                                let name = std::path::Path::new(&path)
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| path.clone());
                                div()
                                    .id(format!("recent-{path}"))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .h(px(28.))
                                    .px(px(6.))
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .overflow_hidden()
                                    .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let _ = root.update(cx, |root, cx| {
                                            root.open_file_path(path.clone(), cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(WEIGHT_MEDIUM)
                                            .text_color(rgba(t.text_primary))
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .child(name),
                                    )
                            }),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = *crate::theme::active(cx);
        let root = cx.entity().downgrade();

        // Caret blink ticker, started once for the entity's lifetime.
        if !self.blink_started {
            self.blink_started = true;
            cx.spawn(async move |weak, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(530))
                        .await;
                    let alive = weak
                        .update(cx, |root, cx| {
                            root.caret_visible = !root.caret_visible;
                            cx.notify();
                        })
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
            })
            .detach();
        }

        let left_dock = self.visible_left.map(|kind| DockView {
            root: root.clone(),
            side: DockSide::Left,
            visible: kind,
            width: self.left_width,
        });
        let right_dock = self.visible_right.map(|kind| DockView {
            root: root.clone(),
            side: DockSide::Right,
            visible: kind,
            width: self.right_width,
        });

        let left_panels = self.panels_on(DockSide::Left);
        let right_panels = self.panels_on(DockSide::Right);

        // Geometry first: everything manuscript-related derives from it.
        let geo = self.geometry(window);
        let measure_color = Hsla::from(rgba(t.text_primary));

        // Snapshot the active buffer once per frame: wrapped visual rows
        // with caret/selection x-edges, history flags, word count.
        // Welcome tabs have no buffer.
        let snapshot: Option<EditorSnapshot> = self
            .tabs
            .get(self.active_tab)
            .filter(|tab| tab.title != "Welcome")
            .map(|tab| tab.id)
            .map(|id| {
                let ed = self.editors.entry(id).or_insert_with(EditorState::new);
                let (cur_line, cur_byte) = ed.cursor_line_byte();
                let sel_chars = ed.selection_range();
                let mut rows = Vec::new();
                let mut caret_placed = false;
                for line_no in 0..ed.len_lines() {
                    let text = ed.line_text(line_no);
                    let ls = ed.line_start_char(line_no);
                    let llen = ed.line_char_len(line_no);
                    let sel_bytes = sel_chars.and_then(|(a, b)| {
                        let o0 = a.max(ls);
                        let o1 = b.min(ls + llen);
                        if o0 >= o1 {
                            None
                        } else {
                            Some((
                                EditorState::col_to_bytes(&text, o0 - ls),
                                EditorState::col_to_bytes(&text, o1 - ls),
                            ))
                        }
                    });
                    for vr in crate::editor::wrap_line(
                        line_no,
                        &text,
                        geo.measure,
                        &mut |s: &str| measure_text(window, s, measure_color),
                    ) {
                        let end = vr.end_byte.min(text.len());
                        let row_text = text[vr.start_byte..end].to_string();
                        // Shape the row once; caret and selection edges
                        // split from this single layout, so kerning matches
                        // the rendered (unsplit) text exactly.
                        let shaped = shape_row(window, &row_text, measure_color);
                        let at = |b: usize| {
                            shaped.split_at(b.min(row_text.len())).0.width().as_f32()
                        };
                        let caret_x = if !caret_placed
                            && line_no == cur_line
                            && vr.start_byte <= cur_byte
                            && cur_byte <= vr.end_byte
                        {
                            caret_placed = true;
                            let rel = cur_byte.min(end).saturating_sub(vr.start_byte);
                            Some(at(rel))
                        } else {
                            None
                        };
                        let sel_px = sel_bytes.and_then(|(s0, s1)| {
                            let o0 = s0.max(vr.start_byte);
                            let o1 = s1.min(vr.end_byte);
                            if o0 >= o1 {
                                None
                            } else {
                                Some((at(o0 - vr.start_byte), at(o1 - vr.start_byte)))
                            }
                        });
                        rows.push(SnapshotRow { text: row_text, caret_x, sel_px });
                    }
                }
                EditorSnapshot {
                    rows,
                    can_undo: ed.can_undo(),
                    can_redo: ed.can_redo(),
                    words: ed.word_count(),
                    has_selection: ed.selection_range().is_some(),
                }
            });
        let can_undo = snapshot.as_ref().is_some_and(|s| s.can_undo);
        let can_redo = snapshot.as_ref().is_some_and(|s| s.can_redo);
        let word_count = snapshot.as_ref().map(|s| s.words).unwrap_or(0);

        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .font_weight(FontWeight::NORMAL)
            .bg(rgba(t.bg_primary))
            .text_color(rgba(t.text_primary))
            .child(TopBar {
                root: root.clone(),
                menu_open: self.menu_open,
                icon_animation: self.icon_animation,
            })
            // Middle row: docks flank the tab bar + manuscript column, and
            // span exactly that height (below top bar, above bottom bar).
            .child(
                div()
                    .id("middle-row")
                    .flex_1()
                    .flex()
                    .flex_row()
                    .min_h(px(0.))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.close_menu(cx);
                        this.close_dock_menu(cx);
                    }))
                    .on_mouse_move(cx.listener(|this, ev: &gpui::MouseMoveEvent, window, cx| {
                        this.drag_to(ev.position.x.as_f32(), cx);
                        this.margin_drag_to(ev.position.x.as_f32(), window, cx);
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                        this.end_drag(cx);
                    }))
                    .when_some(left_dock, |row, dock| row.child(dock))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .min_h(px(0.))
                            .child(TabBar {
                                root: root.clone(),
                                tabs: self.tabs.clone(),
                                active: self.active_tab,
                                hovered: self.hovered_tab,
                                can_undo,
                                can_redo,
                            })
                            .child(self.render_manuscript(
                                root.clone(),
                                snapshot,
                                self.focus.clone(),
                                &geo,
                                window,
                                cx,
                            )),
                    )
                    .when_some(right_dock, |row, dock| row.child(dock)),
            )
            .child(BottomBar {
                root: root.clone(),
                left_panels,
                right_panels,
                visible_left: self.visible_left,
                visible_right: self.visible_right,
                dock_menu: self.dock_menu,
                word_count,
                syntax_on: self.syntax_on,
                prose_on: self.prose_on,
            })
            // Menu overlay, painted last so it sits above the middle row:
            // click-away catcher first, dropdown panel after.
            .when(self.menu_open, |d| {
                let shell = root.clone();
                d.child(
                    div()
                        .absolute()
                        .top(px(TOP_BAR_HEIGHT))
                        .bottom(px(0.))
                        .left(px(0.))
                        .right(px(0.))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let _ = shell.update(cx, |root, cx| root.close_menu(cx));
                        }),
                )
                .child(AppMenu {
                    root: root.clone(),
                    active: self.active_menu,
                    animation: self.menu_animation,
                })
            })
    }
}
