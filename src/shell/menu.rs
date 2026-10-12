//! Top-bar menu model: menus of entries dispatching real actions.

use gpui::{Action, SharedString};

use super::actions::{
    CloseTab, NewTab, OpenFile, Quit, Redo, Save, ShowWelcome, ToggleProjectPanel,
    ToggleStylingPanel, ToggleTheme, Undo,
};

pub struct Menu {
    pub label: SharedString,
    pub items: Vec<MenuItem>,
}

pub struct MenuEntry {
    pub label: SharedString,
    pub shortcut: Option<SharedString>,
    pub action: Box<dyn Action>,
}

pub enum MenuItem {
    Separator,
    Entry(MenuEntry),
}

impl MenuEntry {
    pub fn new(label: impl Into<SharedString>, action: impl Action + Clone + 'static) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
            action: Box::new(action),
        }
    }

}

impl MenuItem {
    pub fn entry(label: impl Into<SharedString>, action: impl Action + Clone + 'static) -> Self {
        Self::Entry(MenuEntry::new(label, action))
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        if let Self::Entry(ref mut entry) = self {
            entry.shortcut = Some(shortcut.into());
        }
        self
    }
}

pub fn default_menus() -> Vec<Menu> {
    vec![
        Menu {
            label: "File".into(),
            items: vec![
                MenuItem::entry("New File", NewTab).shortcut("Ctrl+N"),
                MenuItem::entry("Open File…", OpenFile).shortcut("Ctrl+O"),
                MenuItem::separator(),
                MenuItem::entry("Save", Save).shortcut("Ctrl+S"),
                MenuItem::entry("Close Tab", CloseTab).shortcut("Ctrl+W"),
                MenuItem::separator(),
                MenuItem::entry("Quit", Quit),
            ],
        },
        Menu {
            label: "Edit".into(),
            items: vec![
                MenuItem::entry("Undo", Undo).shortcut("Ctrl+Z"),
                MenuItem::entry("Redo", Redo).shortcut("Ctrl+Shift+Z"),
            ],
        },
        Menu {
            label: "View".into(),
            items: vec![
                MenuItem::entry("Toggle Theme", ToggleTheme),
                MenuItem::separator(),
                MenuItem::entry("Project Panel", ToggleProjectPanel),
                MenuItem::entry("Styles Panel", ToggleStylingPanel),
            ],
        },
        Menu {
            label: "Help".into(),
            items: vec![MenuItem::entry("Welcome Guide", ShowWelcome)],
        },
    ]
}
