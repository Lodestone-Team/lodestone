//! Lodestone Host: the desktop app that runs a Lodestone Core on this computer and
//! manages its servers, rewritten in GPUI after the Tauri app in `dashboard/src-tauri`.
//!
//! - [`core_process`] and [`supervisor`]: the core as a child process;
//! - [`session`]: signing in to it as its owner, through the core's local claim;
//! - [`state`]: what the Host knows about the core, kept live by its event stream;
//! - [`app`]: the window, from starting the core to the shell;
//! - [`shell`], [`routes`] and [`pages`]: the dashboard's layout, navigation and pages;
//! - [`friends`] and [`keychain`]: sharing servers with the friends added in
//!   Lodestone Launcher;
//! - [`tray`] and [`single_instance`]: the menu bar icon and one Host at a time.
//!
//! The components, theme, fonts and the translations' plumbing are shared with
//! Lodestone Launcher (`lodestone-ui`); the core's API is in `launcher_core::cores::admin`.
//! `launcher/docs/host-parity.md` lists what is left to build, by work package.

pub mod app;
pub mod core_process;
pub mod friends;
pub mod i18n;
pub mod instance_icons;
pub mod keychain;
pub mod notifications;
pub mod onboarding;
pub mod pages;
pub mod prefs;
pub mod routes;
pub mod session;
pub mod shell;
pub mod single_instance;
pub mod state;
pub mod supervisor;
pub mod tray;
pub mod updater;

use gpui::{
    App, Bounds, KeyBinding, Menu, MenuItem, SharedString, WindowBounds, WindowOptions, actions,
    px, size,
};

actions!(
    lodestone_host,
    [Quit, ShowWindow, ZoomIn, ZoomOut, ResetZoom, OpenSettings]
);

/// The window's title and the app menu's name.
pub const APP_NAME: &str = "Lodestone Host";

/// Whether this is a development build: the top bar shows a DEV badge, and on macOS the
/// Dock shows the DEV icon, as in the Tauri Host.
pub const IS_DEV: bool = cfg!(debug_assertions);

/// The top bar's height, as the launcher's.
pub const TOP_BAR_HEIGHT: f32 = 42.0;

/// Sets up the fonts, the components, key bindings and the app menu. Call once.
pub fn init(cx: &mut App) {
    // the messages shared with the launcher name this app
    launcher_core::wording::set_app_name(APP_NAME);
    lodestone_ui::init(cx);
    prefs::Prefs::init(cx);
    updater::Updater::init(cx);
    pages::init(cx);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ResetZoom, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("ctrl-=", ZoomIn, None),
        KeyBinding::new("ctrl-+", ZoomIn, None),
        KeyBinding::new("ctrl--", ZoomOut, None),
        KeyBinding::new("ctrl-0", ResetZoom, None),
        KeyBinding::new("ctrl-,", OpenSettings, None),
    ]);
    set_menus(cx);
}

/// The app menu, in the current language.
pub fn set_menus(cx: &mut App) {
    use i18n::{ShellMsg, t};
    cx.set_menus([Menu::new(APP_NAME).items([
        MenuItem::action(t(ShellMsg::SettingsMenu), OpenSettings),
        MenuItem::separator(),
        MenuItem::action(t(ShellMsg::QuitHost), Quit),
    ])]);
    // the Dock icon's menu, for a window closed to the background
    cx.set_dock_menu(vec![MenuItem::action(t(ShellMsg::ShowHost), ShowWindow)]);
}

/// The window: 1100 × 700, at least 900 × 550, as the Tauri Host's. On macOS the title
/// bar is transparent and the top bar sits under the traffic lights; elsewhere the
/// system draws the frame.
pub fn window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1100.), px(700.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(900.), px(550.))),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(SharedString::from(APP_NAME)),
            appears_transparent: cfg!(target_os = "macos"),
            traffic_light_position: cfg!(target_os = "macos")
                .then(|| gpui::point(px(16.), px((TOP_BAR_HEIGHT - 14.0) / 2.0))),
        }),
        app_owns_titlebar_drag: cfg!(target_os = "macos"),
        app_id: Some("dev.lodestone.host".into()),
        ..Default::default()
    }
}
