use super::color::*;
use gpui::{BoxShadow, FontWeight, Point, px, rgba};

pub const FONT_UI: &str = "Geist";
pub const FONT_MONO: &str = "Paper Mono";
// Default manuscript face for the writing surface (overridable in settings).
pub const FONT_PROSE: &str = "Georgia";

/// Emphasis weight for tabs and panel text: heavier than body, lighter
/// than semibold.
pub const WEIGHT_MEDIUM: FontWeight = FontWeight(450.0);

// Fades an RGBA color's alpha byte in from transparent (0xRRGGBBAA colors).
pub fn fade_in(color: u32, k: f32) -> u32 {
    let a = ((color & 0xFF) as f32 * k).round() as u32;
    (color & 0xFFFFFF00) | a
}

// Linear blend of two 0xRRGGBBAA colors (all four channels, including
// the alpha byte). Our palette stores alpha in the low byte, so the
// 6-digit 0xRRGGBB math used elsewhere does NOT apply here.
pub fn lerp_rgba(from: u32, to: u32, k: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xFF) as f32;
    let mix = |shift: u32| (ch(from, shift) + (ch(to, shift) - ch(from, shift)) * k).round() as u32;
    (mix(24) << 24) | (mix(16) << 16) | (mix(8) << 8) | mix(0)
}

// Same as lerp_rgba; kept under the familiar name for call-site parity.
pub fn lerp_rgb(from: u32, to: u32, k: f32) -> u32 {
    lerp_rgba(from, to, k)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg_primary: u32,
    pub bg_secondary: u32,
    pub bg_tertiary: u32,
    pub bg_darker: u32,

    pub border_color: u32,
    pub menu_border_color: u32,
    pub menu_bg: u32,
    pub menu_hover_bg: u32,
    pub component_border_color: u32,
    /// Chrome separators: top/tab/bottom bars and dock edges.
    pub chrome_border_color: u32,

    pub text_primary: u32,
    pub text_secondary: u32,

    pub empty_text_primary: u32,
    pub empty_text_secondary: u32,

    pub shadow_color: u32,
    pub item_shadow_color: u32,

    pub button_background: u32,
    pub button_text: u32,
    pub button_border_color: u32,

    pub accent: u32,
    // Accent-adjacent border for emphasized accent surfaces (chips):
    // darker than accent in light mode, brighter in dark mode.
    pub accent_border: u32,

    // Status tones for diagnostics / proofreading UI.
    pub success: u32,
    pub success_bg: u32,
    pub warning: u32,
    pub warning_bg: u32,
    pub error: u32,
    pub error_bg: u32,
    pub info: u32,
    pub info_bg: u32,

    // History / revision accents (kept free for later revision UI).
    pub vcs_modified: u32,
    pub vcs_added: u32,
    pub vcs_deleted: u32,
    pub vcs_renamed: u32,
    pub vcs_conflict: u32,
    pub vcs_ignored: u32,

    // Revision-comparison accents.
    pub diff_plus: u32,
    pub diff_minus: u32,

    // Editor chrome.
    pub cursor: u32,
    pub selection: u32,
    pub active_line: u32,
    pub line_number: u32,
    pub invisible: u32,
    pub search_match: u32,
    pub search_active_match: u32,
}

impl Theme {
    // Super thin, subtle shadow for popups and floating surfaces.
    pub fn shadow_md(&self) -> BoxShadow {
        BoxShadow {
            color: rgba(self.shadow_color).into(),
            offset: Point {
                x: px(0.),
                y: px(0.75),
            },
            blur_radius: px(3.),
            spread_radius: px(0.),
            inset: false,
        }
    }

    // Barely-there shadow for hovered / active items.
    pub fn shadow_sm(&self) -> BoxShadow {
        BoxShadow {
            color: rgba(self.item_shadow_color).into(),
            offset: Point {
                x: px(0.),
                y: px(0.75),
            },
            blur_radius: px(1.5),
            spread_radius: px(0.),
            inset: false,
        }
    }

    // Large floating shadow for menus and popups.
    pub fn shadow_lg(&self) -> BoxShadow {
        BoxShadow {
            color: rgba(self.shadow_color).into(),
            offset: Point { x: px(0.), y: px(4.) },
            blur_radius: px(16.),
            spread_radius: px(0.),
            inset: false,
        }
    }

    pub fn light() -> Self {
        Self {
            bg_primary: LIGHT_BG_PRIMARY,
            bg_secondary: LIGHT_BG_SECONDARY,
            bg_tertiary: LIGHT_BG_TERTIARY,
            bg_darker: LIGHT_BG_DARKER,

            border_color: LIGHT_BORDER_COLOR,
            menu_border_color: LIGHT_MENU_BORDER_COLOR,
            menu_bg: LIGHT_MENU_BG,
            menu_hover_bg: LIGHT_MENU_HOVER_BG,
            component_border_color: LIGHT_COMPONENT_BORDER_COLOR,
            chrome_border_color: LIGHT_CHROME_BORDER_COLOR,

            text_primary: LIGHT_TEXT_PRIMARY,
            text_secondary: LIGHT_TEXT_SECONDARY,

            empty_text_primary: LIGHT_EMPTY_TEXT_PRIMARY,
            empty_text_secondary: LIGHT_EMPTY_TEXT_SECONDARY,

            shadow_color: LIGHT_SHADOW_COLOR,
            item_shadow_color: LIGHT_ITEM_SHADOW_COLOR,

            button_background: LIGHT_BUTTON_BACKGROUND,
            button_text: LIGHT_BUTTON_TEXT,
            button_border_color: LIGHT_BUTTON_BORDER_COLOR,

            accent: LIGHT_ACCENT,
            accent_border: LIGHT_ACCENT_BORDER,

            success: LIGHT_SUCCESS,
            success_bg: LIGHT_SUCCESS_BG,
            warning: LIGHT_WARNING,
            warning_bg: LIGHT_WARNING_BG,
            error: LIGHT_ERROR,
            error_bg: LIGHT_ERROR_BG,
            info: LIGHT_INFO,
            info_bg: LIGHT_INFO_BG,

            vcs_modified: LIGHT_VCS_MODIFIED,
            vcs_added: LIGHT_VCS_ADDED,
            vcs_deleted: LIGHT_VCS_DELETED,
            vcs_renamed: LIGHT_VCS_RENAMED,
            vcs_conflict: LIGHT_VCS_CONFLICT,
            vcs_ignored: LIGHT_VCS_IGNORED,

            diff_plus: LIGHT_DIFF_PLUS,
            diff_minus: LIGHT_DIFF_MINUS,

            cursor: LIGHT_CURSOR,
            selection: LIGHT_SELECTION,
            active_line: LIGHT_ACTIVE_LINE,
            line_number: LIGHT_LINE_NUMBER,
            invisible: LIGHT_INVISIBLE,
            search_match: LIGHT_SEARCH_MATCH,
            search_active_match: LIGHT_SEARCH_ACTIVE_MATCH,
        }
    }

    pub fn dark() -> Self {
        Self {
            bg_primary: DARK_BG_PRIMARY,
            bg_secondary: DARK_BG_SECONDARY,
            bg_tertiary: DARK_BG_TERTIARY,
            bg_darker: DARK_BG_DARKER,

            border_color: DARK_BORDER_COLOR,
            menu_border_color: DARK_MENU_BORDER_COLOR,
            menu_bg: DARK_MENU_BG,
            menu_hover_bg: DARK_MENU_HOVER_BG,
            component_border_color: DARK_COMPONENT_BORDER_COLOR,
            chrome_border_color: DARK_CHROME_BORDER_COLOR,

            text_primary: DARK_TEXT_PRIMARY,
            text_secondary: DARK_TEXT_SECONDARY,

            empty_text_primary: DARK_EMPTY_TEXT_PRIMARY,
            empty_text_secondary: DARK_EMPTY_TEXT_SECONDARY,

            shadow_color: DARK_SHADOW_COLOR,
            item_shadow_color: DARK_ITEM_SHADOW_COLOR,

            button_background: DARK_BUTTON_BACKGROUND,
            button_text: DARK_BUTTON_TEXT,
            button_border_color: DARK_BUTTON_BORDER_COLOR,

            accent: DARK_ACCENT,
            accent_border: DARK_ACCENT_BORDER,

            success: DARK_SUCCESS,
            success_bg: DARK_SUCCESS_BG,
            warning: DARK_WARNING,
            warning_bg: DARK_WARNING_BG,
            error: DARK_ERROR,
            error_bg: DARK_ERROR_BG,
            info: DARK_INFO,
            info_bg: DARK_INFO_BG,

            vcs_modified: DARK_VCS_MODIFIED,
            vcs_added: DARK_VCS_ADDED,
            vcs_deleted: DARK_VCS_DELETED,
            vcs_renamed: DARK_VCS_RENAMED,
            vcs_conflict: DARK_VCS_CONFLICT,
            vcs_ignored: DARK_VCS_IGNORED,

            diff_plus: DARK_DIFF_PLUS,
            diff_minus: DARK_DIFF_MINUS,

            cursor: DARK_CURSOR,
            selection: DARK_SELECTION,
            active_line: DARK_ACTIVE_LINE,
            line_number: DARK_LINE_NUMBER,
            invisible: DARK_INVISIBLE,
            search_match: DARK_SEARCH_MATCH,
            search_active_match: DARK_SEARCH_ACTIVE_MATCH,
        }
    }
}
