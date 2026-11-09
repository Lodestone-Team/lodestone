// Release builds on Windows are GUI apps: no console window beside the Host.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use futures::StreamExt as _;
use gpui::{AppContext as _, WindowHandle};
use lodestone_host::{
    APP_NAME, Quit, ShowWindow,
    app::HostApp,
    i18n::{ShellMsg, t},
    single_instance,
    tray::{self, TrayCommand},
};
use lodestone_ui::assets::Assets;

fn main() {
    // Before an update renames anything: where this executable is.
    launcher_core::update::remember_exe();
    // Restarted after an update: the old Host holds the single-instance lock until it
    // exits.
    launcher_core::update::wait_for_previous();
    let (show_tx, mut show_rx) = futures::channel::mpsc::unbounded();
    if !single_instance::acquire(show_tx) {
        return;
    }
    let app = gpui_platform::application().with_assets(Assets);
    // a click on the Dock icon brings the window back
    app.on_reopen(|cx| cx.dispatch_action(&ShowWindow));
    app.run(move |cx| {
        lodestone_host::init(cx);
        tray::set_dev_dock_icon();
        let options = lodestone_host::window_options(cx);
        let window: WindowHandle<HostApp> = cx
            .open_window(options, |window, cx| cx.new(|cx| HostApp::new(window, cx)))
            .expect("could not open the Lodestone Host window");
        cx.activate(true);

        let show = move |cx: &mut gpui::App| {
            cx.activate(true);
            window
                .update(cx, |_, window, _| window.activate_window())
                .ok();
        };
        let quit = move |cx: &mut gpui::App| {
            window.update(cx, |app, _, cx| app.quit(cx)).ok();
        };

        cx.on_action(move |_: &Quit, cx| quit(cx));
        cx.on_action(move |_: &ShowWindow, cx| show(cx));

        // the menu bar icon
        let (tray_tx, mut tray_rx) = futures::channel::mpsc::unbounded();
        let tray = tray::install(t(ShellMsg::ShowHost), t(ShellMsg::QuitHost), tray_tx);
        cx.spawn(async move |cx| {
            let _tray = tray;
            while let Some(command) = tray_rx.next().await {
                cx.update(|cx| match command {
                    TrayCommand::Show => show(cx),
                    TrayCommand::Quit => quit(cx),
                });
            }
        })
        .detach();
        // a second start of the Host
        cx.spawn(async move |cx| {
            while show_rx.next().await.is_some() {
                cx.update(|cx| show(cx));
            }
        })
        .detach();
        let _ = APP_NAME;

        // `--screenshot <file.png>` (development): the window, once it has settled
        #[cfg(feature = "screenshot")]
        screenshot::schedule(window, cx);
    });
}

/// `--screenshot <file.png> [--wait <seconds>] [--select-first] [--tab <tab>] [--toasts] [--demo]`,
/// with the `screenshot` feature: renders the window offscreen after the wait (10 s by
/// default: the core starts and the Host signs in), optionally with the first instance
/// selected (on its `--tab`: `console`, `files`, …), sample toasts shown and the page
/// filled with sample data (`--demo`, for the pages that have some), saves it
/// and quits, stopping the core. `--restart-to-update` then restarts into a downloaded
/// update, as Settings › Updates' Restart does.
#[cfg(feature = "screenshot")]
mod screenshot {
    use std::time::Duration;

    use gpui::{App, WindowHandle};
    use lodestone_host::app::HostApp;

    fn arg_after(flag: &str) -> Option<String> {
        let mut args = std::env::args().skip_while(|arg| arg != flag);
        args.next()?;
        args.next()
    }

    pub fn schedule(window: WindowHandle<HostApp>, cx: &mut App) {
        let Some(path) = arg_after("--screenshot") else {
            return;
        };
        let wait = arg_after("--wait")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10);
        let tab = arg_after("--tab").and_then(|name| {
            lodestone_host::routes::InstanceTab::ALL
                .into_iter()
                .find(|tab| format!("{tab:?}").eq_ignore_ascii_case(&name))
        });
        let select_first = tab.is_some() || std::env::args().any(|arg| arg == "--select-first");
        let toasts = std::env::args().any(|arg| arg == "--toasts");
        let demo = std::env::args().any(|arg| arg == "--demo");
        let share = std::env::args().any(|arg| arg == "--share");
        let select_first = select_first || share;
        cx.spawn(async move |cx| {
            // `--owner <username>`: on a first run, fill in the owner account (password
            // from LODESTONE_E2E_PASSWORD) and the core's name, as a user would
            if let Some(username) = arg_after("--owner") {
                let password = std::env::var("LODESTONE_E2E_PASSWORD").unwrap_or_default();
                for _ in 0..120 {
                    let step = window
                        .update(cx, |app, _, cx| {
                            app.onboard_for_screenshot(&username, &password, cx)
                        })
                        .unwrap_or("waiting");
                    eprintln!("first run: {step}");
                    if step == "shell" {
                        break;
                    }
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                }
            }
            cx.background_executor()
                .timer(Duration::from_secs(wait))
                .await;
            if select_first {
                window
                    .update(cx, |app, _, cx| app.select_first_instance(cx))
                    .ok();
                // `--share`: the sidebar's Share button on that instance
                if share {
                    window
                        .update(cx, |app, _, cx| {
                            let shell = app.shell()?;
                            shell.update(cx, |shell, cx| {
                                let lodestone_host::routes::Route::Instance { uuid, .. } =
                                    shell.route.clone()
                                else {
                                    return;
                                };
                                shell.state.update(cx, |state, cx| {
                                    state.share_requested = Some(uuid.clone());
                                    cx.notify();
                                });
                            });
                            Some(())
                        })
                        .ok();
                }
                if let Some(tab) = tab {
                    window
                        .update(cx, |app, _, cx| {
                            let shell = app.shell()?;
                            shell.update(cx, |shell, cx| {
                                if let lodestone_host::routes::Route::Instance { uuid, .. } =
                                    shell.route.clone()
                                {
                                    shell.navigate(
                                        lodestone_host::routes::Route::Instance { uuid, tab },
                                        cx,
                                    );
                                }
                            });
                            Some(())
                        })
                        .ok();
                }
                cx.background_executor()
                    .timer(
                        Duration::from_secs(if tab.is_some() { 2 } else { 0 })
                            + Duration::from_millis(500),
                    )
                    .await;
            }
            // `--settings general|users|profile`: a Settings page
            if let Some(page) = arg_after("--settings").and_then(|name| {
                lodestone_host::routes::SettingsPage::ALL
                    .into_iter()
                    .find(|page| format!("{page:?}").eq_ignore_ascii_case(&name))
            }) {
                window
                    .update(cx, |app, _, cx| {
                        app.shell()?.update(cx, |shell, cx| {
                            shell.navigate(lodestone_host::routes::Route::Settings(page), cx)
                        });
                        Some(())
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(2500))
                    .await;
            }
            // `--cores`: the other cores dialog (package D)
            if std::env::args().any(|arg| arg == "--cores") {
                window
                    .update(cx, |app, _, cx| {
                        app.shell()?.update(cx, |shell, cx| shell.open_cores(cx));
                        Some(())
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
            // `--notifications`: the bell's list, open
            if std::env::args().any(|arg| arg == "--notifications") {
                window
                    .update(cx, |app, _, cx| {
                        let notifications = app.shell()?.read(cx).notifications.clone();
                        notifications.update(cx, |list, cx| list.toggle(cx));
                        Some(())
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
            // `--create <step>`: the "New server" dialog, moved on to `games`, `basic`,
            // `server`, `auto` or `modpack` (package A)
            if let Some(step) = arg_after("--create") {
                window
                    .update(cx, |app, _, cx| {
                        let shell = app.shell()?;
                        shell.update(cx, |shell, cx| shell.open_create(cx));
                        let dialog = shell.read(cx).create_dialog()?;
                        dialog.update(cx, |dialog, cx| dialog.advance_for_screenshot(&step, cx));
                        Some(())
                    })
                    .ok();
                cx.background_executor().timer(Duration::from_secs(4)).await;
            }
            if demo {
                window
                    .update(cx, |app, _, cx| {
                        let view = app.shell()?.read(cx).page_view()?;
                        lodestone_host::pages::demo(&view, cx);
                        Some(())
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
            if toasts {
                use lodestone_ui::components::Toast;
                window
                    .update(cx, |app, _, cx| {
                        let state = app.shell()?.read(cx).state.clone();
                        state.update(cx, |state, cx| {
                            state.toast(Toast::info("Uploading 3 files"), cx);
                            state.toast(Toast::success("Deleted server.properties"), cx);
                            state.toast(Toast::warning("The core is reconnecting"), cx);
                            state.toast(
                                Toast::error("Couldn’t send the command: the server is stopped")
                                    .action("Retry", |_, _| {}),
                                cx,
                            );
                        });
                        Some(())
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
            // `--height <px>`: a taller window, to see a whole page
            if let Some(height) = arg_after("--height").and_then(|h| h.parse::<f32>().ok()) {
                window
                    .update(cx, |_, window, _| {
                        window.resize(gpui::size(gpui::px(1100.), gpui::px(height)))
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
            let save = |path: &str, cx: &mut gpui::AsyncApp| {
                // draw a new frame first: a window in the background may not have
                let image = gpui::AppContext::update_window(cx, window.into(), |_, window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                    window.render_to_image()
                });
                match image.and_then(|image| image) {
                    Ok(image) => match image.save(path) {
                        Ok(()) => eprintln!("saved {path}"),
                        Err(err) => eprintln!("could not save {path}: {err}"),
                    },
                    Err(err) => eprintln!("could not render the window: {err:#}"),
                }
            };
            // `--control <folder>`: keep running, saving the window to the screenshot
            // path every few seconds, until a file named `quit` appears in the folder; a
            // file named `stop-share` presses the Overview's Stop sharing (once)
            if let Some(control) = arg_after("--control").map(std::path::PathBuf::from) {
                let mut stopped_share = false;
                while !control.join("quit").exists() {
                    if !stopped_share && control.join("stop-share").exists() {
                        stopped_share = true;
                        window
                            .update(cx, |app, _, cx| {
                                let view = app.shell()?.read(cx).page_view()?;
                                lodestone_host::pages::stop_share_for_screenshot(&view, cx);
                                Some(())
                            })
                            .ok();
                    }
                    save(&path, cx);
                    cx.background_executor().timer(Duration::from_secs(3)).await;
                }
            }
            save(&path, cx);
            // `--restart-to-update`: with an update downloaded, quit into it as Settings ›
            // Updates' Restart does (to test updating against a local folder)
            if std::env::args().any(|arg| arg == "--restart-to-update") {
                cx.update(|cx| {
                    if let Some(updater) = lodestone_host::updater::Updater::global(cx) {
                        let restarting = updater.update(cx, |u, cx| u.restart(cx));
                        eprintln!("restart to update: {restarting}");
                    }
                });
            }
            window.update(cx, |app, _, cx| app.quit(cx)).ok();
        })
        .detach();
    }
}
