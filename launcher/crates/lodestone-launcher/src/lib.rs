//! Lodestone Launcher, a Minecraft launcher built with GPUI.
//!
//! This crate holds the app shell, the theme carried over from the dashboard and a
//! small set of components. The binary in `main.rs` only opens the window.

pub mod accounts;
pub mod assets;
pub mod components;
/// The control socket for lodestone-launcher-cli. Unix only: Windows has no transport
/// yet (a named pipe would be its equivalent).
#[cfg(unix)]
pub mod control;
pub mod i18n;
pub mod images;
pub mod instances;
pub mod markdown;
#[cfg(feature = "screenshot")]
pub mod preview;
pub mod servers;
pub mod shell;
pub mod theme;

use gpui::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, Render, SharedString, WindowBounds,
    WindowOptions, actions, px, size,
};

actions!(launcher, [Quit, ZoomIn, ZoomOut, ResetZoom]);

/// The window title, also used for the app menu.
pub const APP_NAME: &str = "Lodestone Launcher";

/// Sets up fonts, key bindings and the app menu. Call once, before opening windows.
pub fn init(cx: &mut App) {
    if let Err(err) = assets::load_fonts(cx) {
        // GPUI falls back to the system font for families that did not load.
        eprintln!("lodestone-launcher: could not load the bundled fonts: {err:#}");
    }
    components::init(cx);

    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    // Zoom, as in browsers: ⌘ on macOS (Ctrl works there too), Ctrl elsewhere. The
    // window's root handles the actions, so they reach it from anywhere inside.
    cx.bind_keys([
        KeyBinding::new("ctrl-=", ZoomIn, None),
        KeyBinding::new("ctrl-+", ZoomIn, None),
        KeyBinding::new("ctrl-shift-=", ZoomIn, None),
        KeyBinding::new("ctrl--", ZoomOut, None),
        KeyBinding::new("ctrl-0", ResetZoom, None),
    ]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd-shift-=", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ResetZoom, None),
    ]);
    set_menus(cx);
    // The launcher has one window; closing it quits, as in the dashboard app.
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}

/// Sets the app menu, in the current language. Called again when it changes.
pub fn set_menus(cx: &mut App) {
    let quit = i18n::tr!(i18n::ShellMsg::QuitApp, app = APP_NAME);
    use i18n::{ShellMsg, t};
    cx.set_menus([
        Menu::new(APP_NAME).items([MenuItem::action(quit, Quit)]),
        Menu::new(t(ShellMsg::ViewMenu)).items([
            MenuItem::action(t(ShellMsg::ActualSize), ResetZoom),
            MenuItem::action(t(ShellMsg::ZoomIn), ZoomIn),
            MenuItem::action(t(ShellMsg::ZoomOut), ZoomOut),
        ]),
    ]);
}

/// Options for a launcher window with the given title and size, centred on screen.
///
/// On macOS the title bar is transparent so the app's top bar can sit under the
/// traffic-light buttons, as in the dashboard's Tauri app.
pub fn window_options(title: &str, width: f32, height: f32, cx: &App) -> WindowOptions {
    let title = SharedString::from(title.to_string());
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(720.), px(480.))),
        titlebar: Some(titlebar(title)),
        app_owns_titlebar_drag: cfg!(target_os = "macos"),
        app_id: Some("dev.lodestone.launcher".into()),
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn titlebar(title: SharedString) -> gpui::TitlebarOptions {
    // Centre the 14px-high buttons vertically in the top bar.
    let top = (shell::TOP_BAR_HEIGHT - 14.0) / 2.0;
    gpui::TitlebarOptions {
        title: Some(title),
        appears_transparent: true,
        traffic_light_position: Some(gpui::point(px(16.), px(top))),
    }
}

#[cfg(not(target_os = "macos"))]
fn titlebar(title: SharedString) -> gpui::TitlebarOptions {
    gpui::TitlebarOptions {
        title: Some(title),
        ..Default::default()
    }
}

/// Opens a window with `build` as its root view, and activates the app.
///
/// With the `screenshot` feature and `--screenshot <file.png>` on the command line,
/// the window is rendered to that file once it has settled, and the app quits.
pub fn open_window<V: Render>(
    options: WindowOptions,
    build: impl FnOnce(&mut gpui::Context<V>) -> V + 'static,
    cx: &mut App,
) -> gpui::WindowHandle<V> {
    let window = cx
        .open_window(options, |_, cx| cx.new(build))
        .expect("could not open the launcher window");
    cx.activate(true);

    #[cfg(feature = "screenshot")]
    if let Some(path) = screenshot::requested_path() {
        screenshot::capture(window.into(), path, cx);
    }
    window
}

#[cfg(feature = "screenshot")]
mod screenshot {
    use std::{path::PathBuf, time::Duration};

    use gpui::{AnyWindowHandle, App};

    pub fn requested_path() -> Option<PathBuf> {
        let mut args = std::env::args().skip_while(|arg| arg != "--screenshot");
        args.next()?;
        args.next().map(PathBuf::from)
    }

    /// Renders the window offscreen once images have loaded, writes a PNG and quits.
    /// The native traffic-light buttons are not part of GPUI's scene, so they are
    /// not in the image.
    pub fn capture(window: AnyWindowHandle, path: PathBuf, cx: &mut App) {
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            // Keys the scene presses, to show keyboard focus.
            let keys = crate::preview::requested_scene()
                .map(|scene| crate::preview::keystrokes(&scene))
                .unwrap_or_default();
            for key in keys {
                window
                    .update(cx, |_, window, cx| {
                        if let Ok(keystroke) = gpui::Keystroke::parse(key) {
                            window.dispatch_keystroke(keystroke.clone(), cx);
                            // Draw, so the newly focused control takes the key going up:
                            // GPUI clicks a focused control when Enter or Space goes up.
                            window.draw(cx).clear(cx);
                            window.dispatch_event(
                                gpui::PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke }),
                                cx,
                            );
                            window.draw(cx).clear(cx);
                        }
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
            }
            let result = window.update(cx, |_, window, _| {
                window.refresh();
                window.render_to_image()
            });
            match result.and_then(|image| image) {
                Ok(image) => match image.save(&path) {
                    Ok(()) => eprintln!("saved {}", path.display()),
                    Err(err) => eprintln!("could not save {}: {err}", path.display()),
                },
                Err(err) => eprintln!("could not render the window: {err:#}"),
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}
