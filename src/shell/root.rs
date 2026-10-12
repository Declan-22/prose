//! Root view: top bar, middle row (left dock + tab bar/manuscript +
//! right dock), bottom bar, menu overlay. Owns tabs, dock layout, menus,
//! hover tweens, and drag-resize state.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use gpui::{
    App, Context, FontWeight, IntoElement, MouseButton, Point, Render, WeakEntity, Window, div,
    prelude::*, px, rgba, svg,
};

use super::chrome::{BottomBar, TOP_BAR_HEIGHT, TabBar, TopBar};
use super::dock::{DOCK_DEFAULT_WIDTH, DockView};
use super::dropdown::AppMenu;
use super::panels::{DockSide, PanelKind};
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
    can_undo: bool,
    can_redo: bool,
    recent_files: Vec<String>,
    hovered_tab: Option<usize>,
    store: Store,
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
    pub fn new(store: Store) -> Self {
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
            can_undo: false,
            can_redo: false,
            recent_files: Vec::new(),
            hovered_tab: None,
            store,
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

    // ----- undo / redo (wired to the manuscript buffer next) -----

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        // TODO: route to the manuscript undo stack.
        let _ = cx;
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        // TODO: route to the manuscript redo stack.
        let _ = cx;
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
        self.active_menu = None;
        self.menu_animation = 0.0;
        self.animate_icon(self.menu_open, cx);
        if self.menu_open {
            self.start_menu_animation(cx);
        }
        cx.notify();
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
        self.active_menu = None;
        self.menu_animation = 0.0;
        // Stale hover fades made entries render pre-hovered on reopen.
        self.fades.retain(|k, _| {
            !k.starts_with("menu-entry-") && !k.starts_with("submenu-")
        });
        self.fade_pending
            .retain(|k, _| !k.starts_with("menu-entry-") && !k.starts_with("submenu-"));
        self.fade_tween_active
            .retain(|k| !k.starts_with("menu-entry-") && !k.starts_with("submenu-"));
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
        if self.drag.take().is_some() {
            self.save_layout();
            cx.notify();
        }
    }

    // ----- render -----

    fn render_manuscript(&self, root: WeakEntity<RootView>, cx: &mut App) -> gpui::AnyElement {
        let title = self.tabs.get(self.active_tab).map(|t| t.title.clone()).unwrap_or_default();
        if title == "Welcome" {
            return self.render_welcome(root, cx);
        }
        let t = *crate::theme::active(cx);
        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .items_center()
            .overflow_hidden()
            .bg(rgba(t.bg_primary))
            .cursor_text()
            .child(
                div()
                    .w_full()
                    .max_w(px(640.))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .px(px(32.))
                    .py(px(48.))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgba(t.empty_text_primary))
                            .child(title),
                    )
                    .child(
                        div()
                            .mt(px(16.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgba(t.empty_text_primary))
                            .child("Begin writing…"),
                    ),
            )
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = *crate::theme::active(cx);
        let root = cx.entity().downgrade();

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
                    .on_mouse_move(cx.listener(|this, ev: &gpui::MouseMoveEvent, _, cx| {
                        this.drag_to(ev.position.x.as_f32(), cx);
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
                                can_undo: self.can_undo,
                                can_redo: self.can_redo,
                            })
                            .child(self.render_manuscript(root.clone(), cx)),
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
                word_count: 0,
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
