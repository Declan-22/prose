// Paper theme colors, ported from the Paper Zed theme (light + dark).
// Stored as 0xRRGGBBAA u32 values; convert at the use site with
// `gpui::rgba(...)` (or `gpui::rgb(...)` for fully-opaque colors).

// ========================================
// DARK — "Paper Dark"
// ========================================

pub const DARK_BG_PRIMARY: u32 = 0x20201eff;
pub const DARK_BG_SECONDARY: u32 = 0x1d1d1bff;
pub const DARK_BG_TERTIARY: u32 = 0x242422ff;
pub const DARK_BG_DARKER: u32 = 0x252523ff;

pub const DARK_BORDER_COLOR: u32 = 0x30302eff;
pub const DARK_MENU_BORDER_COLOR: u32 = 0x292927ff;
pub const DARK_COMPONENT_BORDER_COLOR: u32 = 0x3a3a37ff;
// Chrome separators (top/tab/bottom bars, dock edges): a middle ground —
// softer than component borders, more present than menu borders.
pub const DARK_CHROME_BORDER_COLOR: u32 = 0x2e2e2cff;

// Menus stay on the panel tone; hover rows use the element-hover tone.
pub const DARK_MENU_BG: u32 = DARK_BG_SECONDARY;
pub const DARK_MENU_HOVER_BG: u32 = 0x2a2a28ff;

pub const DARK_TEXT_PRIMARY: u32 = 0xd2d2ceff;
pub const DARK_TEXT_SECONDARY: u32 = 0x858580ff;

pub const DARK_EMPTY_TEXT_PRIMARY: u32 = 0x686864ff;
pub const DARK_EMPTY_TEXT_SECONDARY: u32 = 0x555552ff;

pub const DARK_SHADOW_COLOR: u32 = 0x00000040;
pub const DARK_ITEM_SHADOW_COLOR: u32 = 0x00000024;

// High-contrast action buttons (e.g. "+ New document").
pub const DARK_BUTTON_BACKGROUND: u32 = 0xe2e2ddff;
pub const DARK_BUTTON_TEXT: u32 = 0x20201eff;
pub const DARK_BUTTON_BORDER_COLOR: u32 = 0x777772ff;

pub const DARK_ACCENT: u32 = 0x7897caff;
pub const DARK_ACCENT_BORDER: u32 = 0x89a7d8ff;

// Status tones (success / warning / error / info + tints for backgrounds).
pub const DARK_SUCCESS: u32 = 0x819b76ff;
pub const DARK_SUCCESS_BG: u32 = 0x819b7614;
pub const DARK_WARNING: u32 = 0xb99958ff;
pub const DARK_WARNING_BG: u32 = 0xb9995814;
pub const DARK_ERROR: u32 = 0xc17b75ff;
pub const DARK_ERROR_BG: u32 = 0xc17b7514;
pub const DARK_INFO: u32 = 0x7897caff;
pub const DARK_INFO_BG: u32 = 0x7897ca14;

// Version-control / history accents — reused later for revision UI.
pub const DARK_VCS_MODIFIED: u32 = 0x7b8eabff;
pub const DARK_VCS_ADDED: u32 = 0x819b76ff;
pub const DARK_VCS_DELETED: u32 = 0xa87872ff;
pub const DARK_VCS_RENAMED: u32 = 0x927fa0ff;
pub const DARK_VCS_CONFLICT: u32 = 0x9d845aff;
pub const DARK_VCS_IGNORED: u32 = 0x777772ff;

// Diff accents — reused later for revision comparisons.
pub const DARK_DIFF_PLUS: u32 = 0x819b76ff;
pub const DARK_DIFF_MINUS: u32 = 0xc17b75ff;

// Editor chrome.
pub const DARK_CURSOR: u32 = 0x7897caff;
pub const DARK_SELECTION: u32 = 0x7897ca2e;
pub const DARK_ACTIVE_LINE: u32 = 0x29292780;
pub const DARK_LINE_NUMBER: u32 = 0x686864ff;
pub const DARK_INVISIBLE: u32 = 0x484844ff;
pub const DARK_SEARCH_MATCH: u32 = 0x536b8f66;
pub const DARK_SEARCH_ACTIVE_MATCH: u32 = 0x8a744766;

// Extra accent hues for later use (diagnostics, grammar highlighting, charts).
pub const DARK_ACCENT_1: u32 = 0x7698ccff;
pub const DARK_ACCENT_2: u32 = 0xc18a73ff;
pub const DARK_ACCENT_3: u32 = 0x80a980ff;
pub const DARK_ACCENT_4: u32 = 0xa486b8ff;
pub const DARK_ACCENT_5: u32 = 0xb1a367ff;
pub const DARK_ACCENT_6: u32 = 0x6da1a8ff;

// Extra cursor hues for later multi-cursor / collaboration use.
pub const DARK_PLAYER_2: u32 = 0x997bb2ff;
pub const DARK_PLAYER_3: u32 = 0xb99958ff;
pub const DARK_PLAYER_4: u32 = 0x819b76ff;

// ========================================
// LIGHT — "Paper Light"
// ========================================

pub const LIGHT_BG_PRIMARY: u32 = 0xf6f6f3ff;
pub const LIGHT_BG_SECONDARY: u32 = 0xf0f0edff;
pub const LIGHT_BG_TERTIARY: u32 = 0xf3f3f0ff;
pub const LIGHT_BG_DARKER: u32 = 0xf1f1eeff;

pub const LIGHT_BORDER_COLOR: u32 = 0xe5e5e2ff;
pub const LIGHT_MENU_BORDER_COLOR: u32 = 0xecece9ff;
pub const LIGHT_COMPONENT_BORDER_COLOR: u32 = 0xd8d8d4ff;
// Chrome separators (top/tab/bottom bars, dock edges): a middle ground —
// softer than component borders, more present than menu borders.
pub const LIGHT_CHROME_BORDER_COLOR: u32 = 0xe3e3e2ff;

pub const LIGHT_MENU_BG: u32 = LIGHT_BG_SECONDARY;
pub const LIGHT_MENU_HOVER_BG: u32 = 0xecece9ff;

pub const LIGHT_TEXT_PRIMARY: u32 = 0x30302fff;
pub const LIGHT_TEXT_SECONDARY: u32 = 0x858583ff;

pub const LIGHT_EMPTY_TEXT_PRIMARY: u32 = 0xaaa9a6ff;
pub const LIGHT_EMPTY_TEXT_SECONDARY: u32 = 0xb8b8b5ff;

pub const LIGHT_SHADOW_COLOR: u32 = 0x00000012;
pub const LIGHT_ITEM_SHADOW_COLOR: u32 = 0x00000008;

// High-contrast action buttons (e.g. "+ New document").
pub const LIGHT_BUTTON_BACKGROUND: u32 = 0x30302fff;
pub const LIGHT_BUTTON_TEXT: u32 = 0xf6f6f3ff;
pub const LIGHT_BUTTON_BORDER_COLOR: u32 = 0x555553ff;

pub const LIGHT_ACCENT: u32 = 0x5477b8ff;
pub const LIGHT_ACCENT_BORDER: u32 = 0x456ab0ff;

// Status tones (success / warning / error / info + tints for backgrounds).
pub const LIGHT_SUCCESS: u32 = 0x718d67ff;
pub const LIGHT_SUCCESS_BG: u32 = 0x718d6714;
pub const LIGHT_WARNING: u32 = 0xb28a43ff;
pub const LIGHT_WARNING_BG: u32 = 0xb28a4314;
pub const LIGHT_ERROR: u32 = 0xb96f69ff;
pub const LIGHT_ERROR_BG: u32 = 0xb96f6914;
pub const LIGHT_INFO: u32 = 0x5477b8ff;
pub const LIGHT_INFO_BG: u32 = 0x5477b814;

// Version-control / history accents — reused later for revision UI.
pub const LIGHT_VCS_MODIFIED: u32 = 0x7d91b5ff;
pub const LIGHT_VCS_ADDED: u32 = 0x718d67ff;
pub const LIGHT_VCS_DELETED: u32 = 0xad625cff;
pub const LIGHT_VCS_RENAMED: u32 = 0x8a779cff;
pub const LIGHT_VCS_CONFLICT: u32 = 0x9a742fff;
pub const LIGHT_VCS_IGNORED: u32 = 0x999996ff;

// Diff accents — reused later for revision comparisons.
pub const LIGHT_DIFF_PLUS: u32 = 0x718d67ff;
pub const LIGHT_DIFF_MINUS: u32 = 0xb96f69ff;

// Editor chrome.
pub const LIGHT_CURSOR: u32 = 0x5477b8ff;
pub const LIGHT_SELECTION: u32 = 0x5477b82e;
pub const LIGHT_ACTIVE_LINE: u32 = 0xf1f1ee80;
pub const LIGHT_LINE_NUMBER: u32 = 0xa2a2a0ff;
pub const LIGHT_INVISIBLE: u32 = 0xc6c6c3ff;
pub const LIGHT_SEARCH_MATCH: u32 = 0xd9dfeb99;
pub const LIGHT_SEARCH_ACTIVE_MATCH: u32 = 0xe7d9ad99;

// Extra accent hues for later use (diagnostics, grammar highlighting, charts).
pub const LIGHT_ACCENT_1: u32 = 0x6686b8ff;
pub const LIGHT_ACCENT_2: u32 = 0xb17d68ff;
pub const LIGHT_ACCENT_3: u32 = 0x729b72ff;
pub const LIGHT_ACCENT_4: u32 = 0x9474a8ff;
pub const LIGHT_ACCENT_5: u32 = 0xa2945cff;
pub const LIGHT_ACCENT_6: u32 = 0x5d9299ff;

// Extra cursor hues for later multi-cursor / collaboration use.
pub const LIGHT_PLAYER_2: u32 = 0x8a68a6ff;
pub const LIGHT_PLAYER_3: u32 = 0xb28a43ff;
pub const LIGHT_PLAYER_4: u32 = 0x718d67ff;
