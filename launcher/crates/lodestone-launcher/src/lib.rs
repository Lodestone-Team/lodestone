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
pub mod file_dialog;
pub mod glass;
pub mod i18n;
pub mod images;
pub mod instances;
pub mod markdown;
pub mod perf;
#[cfg(feature = "screenshot")]
pub mod preview;
pub mod servers;
pub mod shell;
pub mod skin;
pub mod theme;
mod trace_hooks;
pub mod undo;
pub mod updater;

use gpui::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, Render, SharedString, WindowBounds,
    WindowOptions, actions, px, size,
};

actions!(launcher, [Quit, ZoomIn, ZoomOut, ResetZoom, Undo, Redo]);

/// The window title, also used for the app menu.
pub const APP_NAME: &str = "Lodestone Launcher";

/// Sets up fonts, key bindings and the app menu. Call once, before opening windows.
pub fn init(cx: &mut App) {
    if let Err(err) = assets::load_fonts(cx) {
        // GPUI falls back to the system font for families that did not load.
        eprintln!("lodestone-launcher: could not load the bundled fonts: {err:#}");
    }
    components::init(cx);
    trace_hooks::install(cx);

    cx.on_action(|_: &Quit, cx| cx.quit());
    bind_quit(cx);
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
    // Undo and Redo of settings (see the undo module). A focused text field takes
    // them first, for its text.
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-z", Undo, None),
        KeyBinding::new("cmd-shift-z", Redo, None),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-z", Undo, None),
        KeyBinding::new("ctrl-shift-z", Redo, None),
        KeyBinding::new("ctrl-y", Redo, None),
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

/// Quit's shortcut: ⌘Q on macOS and Ctrl+Q elsewhere, as in browsers and most
/// Linux apps. GPUI reads `cmd` as the Super key on Linux and Windows, so `cmd-q`
/// there would be Super+Q, which desktops keep for themselves. Alt+F4 also works
/// outside macOS: the system closes the window, and the launcher quits with it.
fn bind_quit(cx: &mut App) {
    #[cfg(target_os = "macos")]
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
}

/// Sets the app menu, in the current language. Called again when it changes.
pub fn set_menus(cx: &mut App) {
    let quit = i18n::tr!(i18n::ShellMsg::QuitApp, app = APP_NAME);
    use i18n::{ShellMsg, t};
    cx.set_menus([
        Menu::new(APP_NAME).items([MenuItem::action(quit, Quit)]),
        Menu::new(t(ShellMsg::EditMenu)).items([
            MenuItem::action(t(ShellMsg::Undo), Undo),
            MenuItem::action(t(ShellMsg::Redo), Redo),
        ]),
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
/// traffic-light buttons, as in the dashboard's Tauri app. On Windows the top bar
/// replaces the title bar.
///
/// On Linux the compositor draws the window's frame where it offers to (KDE), and the
/// launcher draws its own where it doesn't (GNOME; see
/// `components::window_frame`). `LODESTONE_LAUNCHER_CLIENT_DECORATIONS=1` asks for the
/// launcher's own frame everywhere, to see it on a desktop that would draw one.
pub fn window_options(title: &str, width: f32, height: f32, cx: &App) -> WindowOptions {
    let title = SharedString::from(title.to_string());
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(720.), px(480.))),
        titlebar: Some(titlebar(title)),
        app_owns_titlebar_drag: cfg!(target_os = "macos"),
        app_id: Some("dev.lodestone.launcher".into()),
        window_decorations: client_decorations_forced().then_some(gpui::WindowDecorations::Client),
        ..Default::default()
    }
}

/// `LODESTONE_LAUNCHER_CLIENT_DECORATIONS=1`, a developer switch: see [`window_options`].
fn client_decorations_forced() -> bool {
    cfg!(any(target_os = "linux", target_os = "freebsd"))
        && std::env::var_os("LODESTONE_LAUNCHER_CLIENT_DECORATIONS")
            .is_some_and(|value| value == "1")
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

/// On Windows the system's title bar is hidden and the top bar takes its place, with
/// its own caption buttons (`shell::top_bar::window_controls`). The title still names
/// the window in the taskbar and Alt-Tab.
#[cfg(not(target_os = "macos"))]
fn titlebar(title: SharedString) -> gpui::TitlebarOptions {
    gpui::TitlebarOptions {
        title: Some(title),
        appears_transparent: cfg!(target_os = "windows"),
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

    // the skin editor's scenes picture the editor's window instead
    #[cfg(feature = "screenshot")]
    if let Some(path) = screenshot::requested_path()
        && !preview::requested_scene()
            .is_some_and(|scene| scene.starts_with("skin-editor") || scene == "perf-monitor")
    {
        screenshot::capture(window.into(), path, cx);
    }
    window
}

/// Where `--screenshot` asks for the picture.
#[cfg(feature = "screenshot")]
pub(crate) fn screenshot_path() -> Option<std::path::PathBuf> {
    screenshot::requested_path()
}

/// Renders `window` to `path` once it has settled, and quits.
#[cfg(feature = "screenshot")]
pub(crate) fn capture_window(
    window: gpui::AnyWindowHandle,
    path: std::path::PathBuf,
    cx: &mut App,
) {
    screenshot::capture(window, path, cx);
}

#[cfg(feature = "screenshot")]
mod screenshot {
    use std::{path::PathBuf, time::Duration};

    use gpui::{AnyWindowHandle, App};

    fn arg_after(flag: &str) -> Option<String> {
        let mut args = std::env::args().skip_while(|arg| arg != flag);
        args.next()?;
        args.next()
    }

    /// `--bench-scroll N [--bench-at X,Y]`: scrolls N steps over the point (the
    /// middle of the window by default), down then up, and prints how long each
    /// frame took to build, lay out and paint.
    fn bench_scroll(window: AnyWindowHandle, steps: usize, cx: &mut gpui::AsyncApp) {
        use gpui::{PlatformInput, ScrollDelta, ScrollEvent, point, px};
        let at = arg_after("--bench-at")
            .and_then(|text| {
                let (x, y) = text.split_once(',')?;
                Some(point(px(x.parse().ok()?), px(y.parse().ok()?)))
            })
            .unwrap_or(point(px(700.), px(450.)));
        let mut frames = Vec::with_capacity(steps);
        for step in 0..steps {
            let down = (step / 40) % 2 == 0;
            window
                .update(cx, |_, window, cx| {
                    let event = ScrollEvent {
                        position: at,
                        delta: ScrollDelta::Pixels(point(
                            px(0.),
                            px(if down { -24. } else { 24. }),
                        )),
                        ..Default::default()
                    };
                    window.dispatch_event(PlatformInput::Scroll(event), cx);
                    let start = std::time::Instant::now();
                    window.draw(cx).clear(cx);
                    frames.push(start.elapsed().as_secs_f64() * 1000.0);
                })
                .ok();
        }
        frames.sort_by(f64::total_cmp);
        if frames.is_empty() {
            return;
        }
        let pick = |q: f64| frames[((frames.len() - 1) as f64 * q) as usize];
        let mean = frames.iter().sum::<f64>() / frames.len() as f64;
        eprintln!(
            "bench-scroll: {} frames, mean {mean:.2} ms, p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms",
            frames.len(),
            pick(0.5),
            pick(0.95),
            pick(1.0)
        );
    }

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
            // A skin's images take longer in a debug build.
            for _ in 0..40 {
                if !cx.update(|cx| crate::skin::decoding(cx)) {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
            }
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
            // `--click X,Y` (repeatable as X,Y;X,Y): left clicks at window points, each
            // followed by a short wait, to check how a control responds.
            if let Some(clicks) = arg_after("--click") {
                for at in clicks.split(';') {
                    let Some((x, y)) = at.split_once(',') else {
                        continue;
                    };
                    let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>()) else {
                        continue;
                    };
                    let position = gpui::point(gpui::px(x), gpui::px(y));
                    window
                        .update(cx, |_, window, cx| {
                            window.dispatch_event(
                                gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                                    position,
                                    ..Default::default()
                                }),
                                cx,
                            );
                            window.draw(cx).clear(cx);
                            window.dispatch_event(
                                gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                                    button: gpui::MouseButton::Left,
                                    position,
                                    modifiers: Default::default(),
                                    click_count: 1,
                                    first_mouse: false,
                                }),
                                cx,
                            );
                            window.dispatch_event(
                                gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                                    button: gpui::MouseButton::Left,
                                    position,
                                    modifiers: Default::default(),
                                    click_count: 1,
                                }),
                                cx,
                            );
                            window.draw(cx).clear(cx);
                        })
                        .ok();
                    cx.background_executor()
                        .timer(Duration::from_millis(600))
                        .await;
                }
            }
            // `--drag X,Y>X,Y` (repeatable with `;`): a left-button drag between window
            // points, as selecting text does. `--right-click X,Y` opens a context menu.
            let point = |at: &str| {
                let (x, y) = at.split_once(',')?;
                Some(gpui::point(
                    gpui::px(x.trim().parse::<f32>().ok()?),
                    gpui::px(y.trim().parse::<f32>().ok()?),
                ))
            };
            if let Some(drags) = arg_after("--drag") {
                for drag in drags.split(';') {
                    let Some((from, to)) = drag
                        .split_once('>')
                        .and_then(|(a, b)| Some((point(a)?, point(b)?)))
                    else {
                        continue;
                    };
                    window
                        .update(cx, |_, window, cx| {
                            window.dispatch_event(
                                gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                                    position: from,
                                    ..Default::default()
                                }),
                                cx,
                            );
                            window.draw(cx).clear(cx);
                            window.dispatch_event(
                                gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                                    button: gpui::MouseButton::Left,
                                    position: from,
                                    modifiers: Default::default(),
                                    click_count: 1,
                                    first_mouse: false,
                                }),
                                cx,
                            );
                            window.draw(cx).clear(cx);
                            for step in 1..=8 {
                                let position = from + (to - from) * (step as f32 / 8.0);
                                window.dispatch_event(
                                    gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                                        position,
                                        pressed_button: Some(gpui::MouseButton::Left),
                                        modifiers: Default::default(),
                                    }),
                                    cx,
                                );
                                window.draw(cx).clear(cx);
                            }
                            window.dispatch_event(
                                gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                                    button: gpui::MouseButton::Left,
                                    position: to,
                                    modifiers: Default::default(),
                                    click_count: 1,
                                }),
                                cx,
                            );
                            window.draw(cx).clear(cx);
                        })
                        .ok();
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                }
            }
            if let Some(position) = arg_after("--right-click").as_deref().and_then(point) {
                window
                    .update(cx, |_, window, cx| {
                        window.dispatch_event(
                            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                                position,
                                ..Default::default()
                            }),
                            cx,
                        );
                        window.draw(cx).clear(cx);
                        window.dispatch_event(
                            gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                                button: gpui::MouseButton::Right,
                                position,
                                modifiers: Default::default(),
                                click_count: 1,
                                first_mouse: false,
                            }),
                            cx,
                        );
                        window.dispatch_event(
                            gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                                button: gpui::MouseButton::Right,
                                position,
                                modifiers: Default::default(),
                                click_count: 1,
                            }),
                            cx,
                        );
                        window.draw(cx).clear(cx);
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
            }
            if let Some(steps) = arg_after("--bench-scroll").and_then(|n| n.parse().ok()) {
                bench_scroll(window, steps, cx);
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

#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{Context, IntoElement, Render, TestAppContext, Window, div};

    struct Empty;

    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// Quit is ⌘Q on macOS and Ctrl+Q elsewhere, where `cmd` would be the Super key.
    #[test]
    fn quit_takes_the_platform_shortcut() {
        let mut cx = TestAppContext::single();
        let quits = Rc::new(Cell::new(0));
        cx.update(|cx| {
            super::bind_quit(cx);
            let quits = quits.clone();
            cx.on_action(move |_: &super::Quit, _| quits.set(quits.get() + 1));
        });
        let (_, cx) = cx.add_window_view(|_, _| Empty);
        let (shortcut, other) = if cfg!(target_os = "macos") {
            ("cmd-q", "ctrl-q")
        } else {
            ("ctrl-q", "cmd-q")
        };
        cx.simulate_keystrokes(other);
        assert_eq!(quits.get(), 0, "{other} doesn't quit");
        cx.simulate_keystrokes(shortcut);
        assert_eq!(quits.get(), 1, "{shortcut} quits");
    }
}
