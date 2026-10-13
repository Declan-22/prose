//! Window chrome: top bar (hamburger menu + window controls), tab bar
//! (undo/redo + tabs, active tab melting into the manuscript), bottom bar
//! (panel icon toggles grouped by dock side + layer toggles).

use gpui::{
    AnyElement, App, FontWeight, MouseButton, WeakEntity, Window, WindowControlArea, div,
    prelude::*, px, rgba, svg,
};

use super::panels::{DockSide, PanelKind, ICON_PROSE_LAYER, ICON_SYNTAX};
use super::root::{RootView, Tab};
use crate::theme::{WEIGHT_MEDIUM, lerp_rgb};

pub const TOP_BAR_HEIGHT: f32 = 30.0;
pub const TAB_BAR_HEIGHT: f32 = 32.0;
pub const BOTTOM_BAR_HEIGHT: f32 = 30.0;

const MENU_ICON: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24">
	<path d="M0 0h24v24H0z" fill="none" />
	<g fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.5">
		<path d="M20 7L4 7" />
		<path d="M20 12L4 12" />
		<path d="M20 17L4 17" />
	</g>
</svg>
"#;

// ---------- Top bar ----------

#[derive(IntoElement)]
pub struct TopBar {
    pub root: WeakEntity<RootView>,
    pub menu_open: bool,
    pub icon_animation: f32,
}

impl gpui::RenderOnce for TopBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);

        div()
            .id("top-bar")
            .w_full()
            .h(px(TOP_BAR_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .bg(rgba(t.bg_primary))
            .border_b_1()
            .border_color(rgba(t.chrome_border_color))
            // Clicking anywhere else on the title bar dismisses the menu.
            .on_mouse_down(MouseButton::Left, {
                let root = self.root.clone();
                move |_, _, cx| {
                    let _ = root.update(cx, |root, cx| root.close_menu(cx));
                }
            })
            .child(render_menu_button(
                self.menu_open,
                self.icon_animation,
                t,
                self.root.clone(),
                cx,
            ))
            // Spacer — only this region is draggable so it doesn't
            // swallow the menu or close / min / max hitboxes.
            .child(div().flex_1().h_full().window_control_area(WindowControlArea::Drag))
            .child(render_window_controls(t, cx))
    }
}

fn render_menu_button(
    _menu_open: bool,
    animation: f32,
    t: crate::theme::Theme,
    root: WeakEntity<RootView>,
    cx: &App,
) -> AnyElement {
    // Active fade follows the open/close animation; hover fade is a tween.
    let act = animation.clamp(0., 1.);
    let hov = shell_fade(&root, cx, "tb-menu");
    let k = act.max(hov);
    let bg = crate::theme::lerp_rgb(t.bg_primary, t.bg_tertiary, k);
    let icon_color = crate::theme::lerp_rgb(t.text_secondary, t.text_primary, act);
    let border = crate::theme::fade_in(t.border_color, k);
    let mut shadow = t.shadow_sm();
    shadow.color = rgba(crate::theme::fade_in(t.item_shadow_color, k)).into();

    div()
        .id("app-menu-button")
        .flex()
        .items_center()
        .justify_center()
        .w(px(24.))
        .h(px(24.))
        .ml(px(6.))
        .rounded(px(6.))
        .cursor_pointer()
        // Constant geometry; only colors tween.
        .border_1()
        .border_color(rgba(border))
        .bg(rgba(bg))
        .shadow(vec![shadow])
        .on_hover({
            let root = root.clone();
            move |hovered, _, cx| {
                let _ = root.update(cx, |root, cx| {
                    root.animate_fade("tb-menu", if *hovered { 1.0 } else { 0.0 }, cx);
                });
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = root.update(cx, |root, cx| {
                root.toggle_menu(cx);
                cx.stop_propagation();
            });
        })
        .child(
            svg()
                .data(MENU_ICON)
                .w(px(16.))
                .h(px(16.))
                .text_color(rgba(icon_color)),
        )
        .into_any_element()
}

fn render_window_controls(t: crate::theme::Theme, _cx: &App) -> AnyElement {
    let control = |id: &'static str, area: WindowControlArea, content: AnyElement| -> AnyElement {
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .window_control_area(area)
            .hover(|s| s.bg(rgba(t.menu_hover_bg)))
            .child(content)
            .into_any_element()
    };

    div()
        .flex()
        .flex_row()
        .items_stretch()
        .h_full()
        .child(control(
            "window-minimize",
            WindowControlArea::Min,
            div().w(px(10.)).h(px(1.)).bg(rgba(t.text_secondary)).into_any_element(),
        ))
        .child(control(
            "window-maximize",
            WindowControlArea::Max,
            div()
                .size(px(9.))
                .border_1()
                .border_color(rgba(t.text_secondary))
                .into_any_element(),
        ))
        .child(control(
            "window-close",
            WindowControlArea::Close,
            div().text_sm().text_color(rgba(t.text_primary)).child("✕").into_any_element(),
        ))
        .into_any_element()
}

// Reads a fade value from the root without needing a full upgrade.
fn shell_fade(root: &WeakEntity<RootView>, cx: &App, key: &str) -> f32 {
    root.upgrade().map(|s| s.read(cx).fade(key)).unwrap_or(0.0)
}

// ---------- Tab bar ----------

#[derive(IntoElement)]
pub struct TabBar {
    pub root: WeakEntity<RootView>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub hovered: Option<usize>,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl gpui::RenderOnce for TabBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);
        // The bar itself carries no bottom border. Inactive tabs, the
        // history cluster, the + button and the filler each draw their own
        // 1px bottom edge — the active tab draws none, so it melts into
        // the manuscript surface below.
        let edge = rgba(t.chrome_border_color);

        let undo = |id: &'static str, label: &'static str, enabled: bool| {
            let root = self.root.clone();
            div()
                .id(id)
                .w(px(30.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(rgba(if enabled {
                    t.text_secondary
                } else {
                    t.empty_text_secondary
                }))
                .hover(|s| {
                    if enabled {
                        s.bg(rgba(t.menu_hover_bg))
                    } else {
                        s
                    }
                })
                .on_mouse_down(MouseButton::Left, {
                    let root = root.clone();
                    move |_, _, cx| {
                        let _ = root.update(cx, |root, cx| {
                            if id == "tab-undo" {
                                root.undo(cx);
                            } else {
                                root.redo(cx);
                            }
                        });
                    }
                })
                .child(label)
        };

        div()
            .id("tab-bar")
            .w_full()
            .h(px(TAB_BAR_HEIGHT))
            .flex()
            .flex_row()
            .items_stretch()
            .bg(rgba(t.bg_secondary))
            // Undo / redo sit all the way left; tabs follow to the right.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_stretch()
                    .border_b_1()
                    .border_color(edge)
                    .child(undo("tab-undo", "↩", self.can_undo))
                    .child(undo("tab-redo", "↪", self.can_redo))
                    // Full-height divider between history buttons and tabs.
                    // Goes invisible when the first tab is active — its own
                    // left border takes over, so the seam never doubles.
                    .child(div().w(px(1.)).h_full().bg(if self.active == 0 {
                        rgba(0x00000000)
                    } else {
                        edge
                    })),
            )
            .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                let root = self.root.clone();
                let tab_id = tab.id;
                let is_active = i == self.active;
                let is_hovered = self.hovered == Some(tab_id);
                // Constant geometry in both states: the outer shell always
                // carries side borders, the inner row always carries the
                // bottom border — only colors change. Inactive sides are
                // transparent and the active bottom edge is manuscript-toned,
                // so switching tabs never moves anything by a pixel.
                div()
                    .id(format!("tab-{tab_id}"))
                    .h_full()
                    .flex()
                    .flex_row()
                    .items_stretch()
                    .cursor_pointer()
                    .border_l_1()
                    .border_r_1()
                    .border_color(rgba(if is_active {
                        t.chrome_border_color
                    } else {
                        0x00000000
                    }))
                    .bg(if is_active {
                        rgba(t.bg_primary)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|s| {
                        if is_active {
                            s
                        } else {
                            s.bg(rgba(t.menu_hover_bg))
                        }
                    })
                    .on_hover({
                        let root = root.clone();
                        move |hovered, _, cx| {
                            let _ = root.update(cx, |root, cx| {
                                root.set_hovered_tab(
                                    if *hovered { Some(tab_id) } else { None },
                                    cx,
                                );
                            });
                        }
                    })
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = root.update(cx, |root, cx| root.activate_tab(tab_id, cx));
                    })
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .pl(px(23.))
                            .pr(px(24.))
                            .border_b_1()
                            .border_color(rgba(if is_active {
                                t.bg_primary
                            } else {
                                t.chrome_border_color
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .text_center()
                                    .text_sm()
                                    .font_weight(WEIGHT_MEDIUM)
                                    .text_color(rgba(if is_active {
                                        t.text_primary
                                    } else {
                                        t.text_secondary
                                    }))
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .child(tab.title.clone()),
                            )
                            .child({
                    let root = self.root.clone();
                    // Fixed slot so the title never shifts; the × itself
                    // only appears while hovering its tab. Absolutely
                    // positioned over the right padding so the title stays
                    // truly centered. Hover recolors it — no background.
                    div()
                        .absolute()
                        .right(px(2.))
                        .top(px(0.))
                        .bottom(px(0.))
                        .w(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(is_hovered, |slot| {
                            slot.child(
                                div()
                                    .id(format!("tab-close-{tab_id}"))
                                    .w(px(16.))
                                    .h(px(16.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.))
                                    .text_sm()
                                    .text_color(rgba(t.text_secondary))
                                    .hover(|s| s.text_color(rgba(t.text_primary)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let _ = root.update(cx, |root, cx| {
                                            root.close_tab(tab_id, cx)
                                        });
                                        cx.stop_propagation();
                                    })
                                    .child("×"),
                            )
                        })
                }))
            }))
            // Filler carries the border across the remaining width.
            .child(div().flex_1().border_b_1().border_color(edge))
    }
}

// ---------- Bottom bar ----------

#[derive(IntoElement)]
pub struct BottomBar {
    pub root: WeakEntity<RootView>,
    pub left_panels: Vec<PanelKind>,
    pub right_panels: Vec<PanelKind>,
    pub visible_left: Option<PanelKind>,
    pub visible_right: Option<PanelKind>,
    pub dock_menu: Option<PanelKind>,
    pub word_count: usize,
    pub syntax_on: bool,
    pub prose_on: bool,
    pub syntax_menu_open: bool,
    pub prose_menu_open: bool,
    pub syntax_selected: [bool; 5],
    pub prose_selected: [bool; 8],
}

impl gpui::RenderOnce for BottomBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let t = *crate::theme::active(cx);

        let toggle = |kind: PanelKind, is_visible: bool, popup_open: bool| {
            let root = self.root.clone();
            let icon_color = if is_visible { t.accent } else { t.text_secondary };
            div()
                .relative()
                .child(
                    div()
                        .id(format!("panel-toggle-{}", kind.short_label()))
                        .w(px(26.))
                        .h(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .bg(if popup_open {
                            rgba(t.menu_hover_bg)
                        } else {
                            rgba(0x00000000)
                        })
                        .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                        // Left-click toggles visibility in its dock.
                        .on_mouse_down(MouseButton::Left, {
                            let root = root.clone();
                            move |_, _, cx| {
                                let _ = root.update(cx, |root, cx| root.toggle_panel(kind, cx));
                            }
                        })
                        // Right-click opens the Dock Left / Dock Right popup.
                        .on_mouse_down(MouseButton::Right, {
                            let root = root.clone();
                            move |_, _, cx| {
                                let _ = root.update(cx, |root, cx| root.open_dock_menu(kind, cx));
                                cx.stop_propagation();
                            }
                        })
                        .child(
                            svg()
                                .data(kind.icon())
                                .w(px(14.))
                                .h(px(14.))
                                .text_color(rgba(icon_color)),
                        ),
                )
                .when(popup_open, |w| {
                    w.child(dock_popup(kind, self.root.clone(), &self.left_panels, t, cx))
                })
        };

        let layer = |id: &'static str, label: &'static str, icon: &'static [u8], on: bool, flag: u8, open: bool, selected: Vec<bool>| {
            let root = self.root.clone();
            div()
                .relative()
                .id(id)
                .h(px(26.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(5.))
                .px(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .text_xs()
                .font_weight(FontWeight::NORMAL)
                .text_color(rgba(if on { t.accent } else { t.text_secondary }))
                .hover(|s| s.bg(rgba(t.menu_hover_bg)))
                .on_mouse_down(MouseButton::Left, {
                    let root = root.clone();
                    move |_, _, cx| {
                        let _ = root.update(cx, |root, cx| root.toggle_layer(flag, cx));
                        cx.stop_propagation();
                    }
                })
                .child(
                    svg()
                        .data(icon)
                        .w(px(13.))
                        .h(px(13.))
                        .text_color(rgba(if on { t.accent } else { t.text_secondary })),
                )
                .child(label)
                .when(open, |w| w.child(layer_popup(self.root.clone(), flag, selected, t, cx)))
        };

        div()
            .id("bottom-bar")
            .w_full()
            .h(px(BOTTOM_BAR_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .px(px(6.))
            .gap(px(2.))
            .bg(rgba(t.bg_secondary))
            .border_t_1()
            .border_color(rgba(t.chrome_border_color))
            .on_mouse_down(MouseButton::Left, {
                let root = self.root.clone();
                move |_, _, cx| {
                    let _ = root.update(cx, |root, cx| { root.close_dock_menu(cx); root.close_layer_menus(cx); });
                }
            })
            // Left group: panels docked left.
            .children(self.left_panels.iter().map(|k| {
                toggle(*k, Some(*k) == self.visible_left, Some(*k) == self.dock_menu)
            }))
            .child(div().flex_1())
            // Center: layer toggles + word count.
            .child(layer("layer-syntax", "Syntax", ICON_SYNTAX, self.syntax_on, 0, self.syntax_menu_open, self.syntax_selected.to_vec()))
            .child(layer("layer-prose", "Prose", ICON_PROSE_LAYER, self.prose_on, 1, self.prose_menu_open, self.prose_selected.to_vec()))
            .child(
                div()
                    .px(px(8.))
                    .text_xs()
                    .font_weight(FontWeight::NORMAL)
                    .text_color(rgba(t.text_secondary))
                    .child(format!("{} words", self.word_count)),
            )
            .child(div().flex_1())
            // Right group: panels docked right.
            .children(self.right_panels.iter().map(|k| {
                toggle(*k, Some(*k) == self.visible_right, Some(*k) == self.dock_menu)
            }))
    }
}

fn layer_popup(root: WeakEntity<RootView>, layer: u8, selected: Vec<bool>, t: crate::theme::Theme, cx: &App) -> AnyElement {
    let labels: Vec<&'static str> = if layer == 0 {
        crate::syntax::Kind::ALL.iter().map(|k| k.label()).collect()
    } else {
        crate::prose::Kind::ALL.iter().map(|k| k.label()).collect()
    };
    div()
        .absolute()
        .bottom(px(30.))
        .left(px(0.))
        .w(px(190.))
        .px(px(4.))
        .py(px(4.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .bg(rgba(t.bg_primary))
        .border_1()
        .border_color(rgba(t.menu_border_color))
        .rounded(px(10.))
        .shadow(vec![t.shadow_lg()])
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .children(labels.into_iter().enumerate().map(|(index, label)| {
            let root = root.clone();
            let on = selected.get(index).copied().unwrap_or(false);
            let fade_id = format!("layerpopup-{layer}-{index}");
            let k = root.upgrade().map(|s| s.read(cx).fade(&fade_id)).unwrap_or(0.0);
            let bg = lerp_rgb(t.bg_primary, t.menu_hover_bg, k);
            let color = if layer == 0 {
                [t.accent, t.success, t.warning, t.error, t.info][index]
            } else {
                [t.warning, t.error, t.info, t.accent, t.success, t.warning, t.info, t.error][index]
            };
            div()
                .id(format!("layer-check-{layer}-{index}"))
                .h(px(24.))
                .pl(px(4.))
                .pr(px(4.))
                .gap(px(8.))
                .flex()
                .flex_row()
                .items_center()
                .rounded(px(8.))
                .cursor_pointer()
                .text_sm()
                .font_weight(WEIGHT_MEDIUM)
                .text_color(rgba(t.text_primary))
                .bg(rgba(bg))
                .opacity(if on { 1.0 } else { 0.42 })
                .on_hover({
                    let root = root.clone();
                    let fade_id = fade_id.clone();
                    move |hovered, _, cx| {
                        let _ = root.update(cx, |root, cx| {
                            root.animate_fade(&fade_id, if *hovered { 1.0 } else { 0.0 }, cx);
                        });
                    }
                })
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let _ = root.update(cx, |root, cx| root.toggle_check(layer, index, cx));
                    cx.stop_propagation();
                })
                .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(rgba(color)))
                .child(label)
        }))
        .into_any_element()
}

/// Dock Left / Dock Right popup, anchored above its bottom-bar button.
/// Styled identically to the top-bar dropdown: same panel, same rows,
/// same background-only hover tweens. The active side shows a filled dot.
fn dock_popup(
    kind: PanelKind,
    root: WeakEntity<RootView>,
    left_panels: &[PanelKind],
    t: crate::theme::Theme,
    cx: &App,
) -> AnyElement {
    let current = if left_panels.contains(&kind) {
        DockSide::Left
    } else {
        DockSide::Right
    };
    let row = |label: &'static str, side: DockSide| {
        let root = root.clone();
        let is_current = current == side;
        // Hover tween, background-only like the dropdown entries.
        let k = root
            .upgrade()
            .map(|s| s.read(cx).fade(&format!("dockpopup-{label}")))
            .unwrap_or(0.0);
        let bg = lerp_rgb(t.bg_primary, t.menu_hover_bg, k);
        div()
            .id(format!("dock-{label}-{}", kind.short_label()))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(24.))
            .pl(px(4.))
            .pr(px(4.))
            .rounded(px(8.))
            .cursor_pointer()
            .text_sm()
            .font_weight(WEIGHT_MEDIUM)
            .text_color(rgba(t.text_primary))
            .bg(rgba(bg))
            .on_hover({
                let root = root.clone();
                move |hovered, _, cx| {
                    let _ = root.update(cx, |root, cx| {
                        root.animate_fade(
                            &format!("dockpopup-{label}"),
                            if *hovered { 1.0 } else { 0.0 },
                            cx,
                        );
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = root.update(cx, |root, cx| {
                    root.set_dock(kind, side, cx);
                    root.close_dock_menu(cx);
                });
                cx.stop_propagation();
            })
            .child(label)
            .child(
                div()
                    .w(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(is_current, |slot| {
                        slot.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(rgba(t.accent)))
                    }),
            )
    };

    div()
        .absolute()
        .bottom(px(BOTTOM_BAR_HEIGHT))
        .left(px(0.))
        .w(px(140.))
        .flex()
        .flex_col()
        .px(px(4.))
        .py(px(4.))
        .gap(px(2.))
        .bg(rgba(t.bg_primary))
        .border_1()
        .border_color(rgba(t.menu_border_color))
        .rounded(px(10.))
        .shadow(vec![t.shadow_lg()])
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(row("Dock Left", DockSide::Left))
        .child(row("Dock Right", DockSide::Right))
        .into_any_element()
}
