use std::borrow::Cow;

use gpui::{App, AppContext, Bounds, KeyBinding, WindowBounds, WindowOptions, px, size};

use gpui_platform::application;

use crate::persist::Store;
use crate::shell::RootView;
use crate::shell::actions::*;
use crate::shell::panels::PanelKind;
use crate::theme::{self, ThemeMode};

pub fn run() {
    application().run(|cx: &mut App| {
        load_fonts(cx);
        let store = Store::open_default();
        let saved_mode = store.get("theme.mode").map(|v| match v.as_str() {
            "light" => ThemeMode::Light,
            _ => ThemeMode::Dark,
        });
        theme::init(cx, saved_mode);
        register_action_handlers(cx);
        bind_keys(cx);

        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(800.), px(500.))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Prose".into()),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |window, cx| {
                let store = Store::open_default();
                let view = cx.new(|cx| RootView::new(cx, store));
                // Manuscript takes keyboard focus immediately; clicking it
                // re-focuses later.
                let focus = view.read(cx).focus.clone();
                window.focus(&focus, cx);
                cx.set_global(RootHandle(view.downgrade()));
                view
            },
        )
        .unwrap();

        cx.activate(true);
    });
}

fn load_fonts(cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/Geist-VariableFont_wght.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/PaperMono-Regular.ttf")),
    ];
    cx.text_system()
        .add_fonts(fonts)
        .expect("failed to load bundled fonts");
}

fn with_root(cx: &mut App, f: impl FnOnce(&mut RootView, &mut gpui::Context<RootView>)) {
    let handle = cx.global::<RootHandle>().0.clone();
    let _ = handle.update(cx, f);
}

fn register_action_handlers(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &ToggleTheme, cx| {
        with_root(cx, |root, cx| root.toggle_theme(cx));
    });
    cx.on_action(|_: &NewTab, cx| {
        with_root(cx, |root, cx| root.new_tab(cx));
    });
    cx.on_action(|_: &CloseTab, cx| {
        with_root(cx, |root, cx| root.close_active_tab(cx));
    });
    cx.on_action(|_: &Undo, cx| {
        with_root(cx, |root, cx| root.undo(cx));
    });
    cx.on_action(|_: &Redo, cx| {
        with_root(cx, |root, cx| root.redo(cx));
    });
    cx.on_action(|_: &ShowWelcome, cx| {
        with_root(cx, |root, cx| root.open_welcome(cx));
    });
    cx.on_action(|_: &ToggleProjectPanel, cx| {
        with_root(cx, |root, cx| root.toggle_panel(PanelKind::Project, cx));
    });
    cx.on_action(|_: &ToggleStylingPanel, cx| {
        with_root(cx, |root, cx| root.toggle_panel(PanelKind::Styling, cx));
    });
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-n", NewTab, None),
        KeyBinding::new("ctrl-w", CloseTab, None),
        KeyBinding::new("ctrl-z", Undo, None),
        KeyBinding::new("ctrl-shift-z", Redo, None),
    ]);
}
