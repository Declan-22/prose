//! Left/right docks: free-docking panel containers with drag resizing.
//! Content never reflows when a dock shrinks — it simply crops, like Zed.
//! The resize handle is an invisible overlay straddling the dock's inner
//! edge, so the tab bar meets the dock with no gap.

use gpui::{
    AnyElement, App, CursorStyle, MouseButton, MouseDownEvent, WeakEntity, div, prelude::*, px,
    rgba,
};

use super::panels::{DockSide, PanelKind};
use super::root::RootView;
use crate::theme::WEIGHT_MEDIUM;

pub const DOCK_DEFAULT_WIDTH: f32 = 240.0;
pub const DOCK_HANDLE_WIDTH: f32 = 10.0;

#[derive(IntoElement)]
pub struct DockView {
    pub root: WeakEntity<RootView>,
    pub side: DockSide,
    pub visible: PanelKind,
    pub width: f32,
}

impl DockView {
    fn divider_id(&self) -> &'static str {
        match self.side {
            DockSide::Left => "dock-divider-left",
            DockSide::Right => "dock-divider-right",
        }
    }

    fn render_body(&self, cx: &mut App) -> AnyElement {
        let t = *crate::theme::active(cx);
        let muted = |s: &'static str| {
            div()
                .pl(px(8.))
                .text_sm()
                .font_weight(WEIGHT_MEDIUM)
                .text_color(rgba(t.empty_text_primary))
                .whitespace_nowrap()
                .overflow_hidden()
                .child(s)
        };
        // Rows run edge to edge — no right-side inset, so text crops
        // exactly at the dock border.
        match self.visible {
            PanelKind::Project => div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .overflow_hidden()
                .child(muted("No folder opened yet."))
                .into_any_element(),
            PanelKind::Outline => div()
                .overflow_hidden()
                .child(muted("No headings yet — headings appear here."))
                .into_any_element(),
            PanelKind::Styling => {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(4.))
                            .pl(px(8.))
                            .child(style_button("B"))
                            .child(style_button("I"))
                            .child(style_button("U")),
                    )
                    .children(["Emphasis", "Research note", "Needs revision"].into_iter().map(|name| {
                        let root = self.root.clone();
                        let key = match name {
                            "Emphasis" => "E",
                            "Research note" => "R",
                            _ => "X",
                        };
                        div()
                            .id(format!("style-row-{key}"))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .overflow_hidden()
                            .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                            .on_mouse_down(MouseButton::Left, {
                                let root = root.clone();
                                move |_, _, cx| {
                                    let _ = root.update(cx, |root, cx| {
                                        root.toggle_preview_style(name, cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(WEIGHT_MEDIUM)
                                    .text_color(rgba(t.text_primary))
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .child(name.to_string()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(WEIGHT_MEDIUM)
                                    .text_color(rgba(t.text_secondary))
                                    .child(key.to_string()),
                            )
                    }))
                    .into_any_element()
            }
            PanelKind::Inspector => div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .overflow_hidden()
                .child(muted("Words —"))
                .child(muted("Reading time —"))
                .child(muted("Diagnostics appear while you revise."))
                .into_any_element(),
            PanelKind::History => div()
                .overflow_hidden()
                .child(muted("No snapshots yet."))
                .into_any_element(),
            PanelKind::Comments => div()
                .overflow_hidden()
                .child(muted("No comment pins yet."))
                .into_any_element(),
        }
    }
}

fn style_button(label: &'static str) -> AnyElement {
    // Character buttons are inert until direct formatting lands; they only
    // need to exist in layout for now.
    div()
        .w(px(28.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .text_sm()
        .font_weight(WEIGHT_MEDIUM)
        .child(label)
        .into_any_element()
}

impl gpui::RenderOnce for DockView {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        let root = self.root.clone();
        let side = self.side;

        // Invisible overlay straddling the dock's inner edge: 2px over the
        // dock, 3px over the manuscript. No hover effect — the cursor
        // change is the only affordance, and the tab bar meets the dock
        // edge with zero gap.
        let handle = div()
            .id(self.divider_id())
            .absolute()
            .top(px(0.))
            .h_full()
            .w(px(DOCK_HANDLE_WIDTH))
            .cursor(CursorStyle::ResizeLeftRight)
            .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                let max = window.viewport_size().width.as_f32();
                let _ = root.update(cx, |root, cx| {
                    root.begin_dock_drag(side, ev.position.x.as_f32(), max, cx);
                });
            });
        let handle = match side {
            DockSide::Left => handle.right(px(-4.)),
            DockSide::Right => handle.left(px(-4.)),
        };

        let panel = div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgba(t.bg_secondary))
            // 1px separator where the dock meets the tab bar / manuscript.
            .border_color(rgba(t.chrome_border_color))
            .when(side == DockSide::Left, |p| p.border_r_1())
            .when(side == DockSide::Right, |p| p.border_l_1())
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .pt(px(8.))
                    .pb(px(8.))
                    .overflow_hidden()
                    .child(self.render_body(cx)),
            );

        // Fixed-width slot; the handle floats above the seam and adds no
        // layout width, so dock + tab bar always touch exactly.
        div()
            .relative()
            .w(px(self.width.max(0.0)))
            .h_full()
            .child(panel)
            .child(handle)
            .into_any_element()
    }
}
