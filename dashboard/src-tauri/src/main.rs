#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use lodestone_core::AppState;

use lodestone_core::error::Error;

use lodestone_core::auth::jwt_token::JwtToken;
use lodestone_core::tauri_export::is_owner_account_present;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
#[cfg(not(dev))]
use tauri::{utils::config::FrontendDist, Url};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::UpdaterExt;

use notify_rust::Notification;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
#[derive(Clone, serde::Serialize)]
struct Payload {
    args: Vec<String>,
    cwd: String,
}

#[tauri::command]
async fn is_setup(state: tauri::State<'_, AppState>) -> Result<bool, ()> {
    Ok(is_owner_account_present(state.inner()).await)
}

#[tauri::command]
async fn setup_owner_account(
    state: tauri::State<'_, AppState>,
    username: String,
    password: String,
) -> Result<(), Error> {
    lodestone_core::tauri_export::setup_owner_account(state.inner(), username, password).await
}

#[tauri::command]
async fn get_first_time_setup_key(state: tauri::State<'_, AppState>) -> Result<String, ()> {
    lodestone_core::tauri_export::get_first_time_setup_key(state.inner())
        .await
        .ok_or(())
}

#[tauri::command]
async fn get_owner_jwt(state: tauri::State<'_, AppState>) -> Result<JwtToken, ()> {
    lodestone_core::tauri_export::get_owner_jwt(state.inner())
        .await
        .ok_or(())
}

/// Whether this is a debug (local development) build. The dashboard shows a DEV badge for it.
#[tauri::command]
fn is_debug_build() -> bool {
    cfg!(debug_assertions)
}

/// Debug builds use an icon with a "DEV" ribbon so they are easy to tell apart from an
/// installed release. On macOS the Dock icon comes from the app bundle, so it is replaced at
/// runtime, the same way Tauri sets the icon for `tauri dev`.
#[cfg(all(debug_assertions, target_os = "macos"))]
fn set_dev_dock_icon() {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    // Only called from the Ready event, which runs on the main thread.
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(include_bytes!("../icons/dev/icon-macos.png"));
    if let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) {
        let app = NSApplication::sharedApplication(mtm);
        unsafe { app.setApplicationIconImage(Some(&icon)) };
    }
}

/// Shows a native Yes/No dialog over the main window and returns whether the user chose Yes.
async fn ask(app: &AppHandle, title: String, message: String) -> bool {
    let app = app.clone();
    // blocking_show must not run on the main thread or block an async worker.
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app
            .dialog()
            .message(message)
            .title(title)
            .buttons(MessageDialogButtons::YesNo);
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.parent(&window);
        }
        dialog.blocking_show()
    })
    .await
    .unwrap_or(false)
}

/// Tauri 2 removed the built-in updater dialog (v1 `updater.dialog: true`).
/// This does the same as v1: check on startup, ask before installing, then ask to restart.
async fn check_update_with_dialog(app: AppHandle) -> tauri_plugin_updater::Result<()> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(());
    };
    let app_name = app.package_info().name.clone();
    let body = update.body.clone().unwrap_or_default();

    let should_install = ask(
        &app,
        format!("A new version of {app_name} is available! "),
        format!(
            "{app_name} {} is now available -- you have {}.\n\nWould you like to install it now?\n\nRelease Notes:\n{body}",
            update.version, update.current_version
        ),
    )
    .await;

    if should_install {
        // On Windows the installer exits the app here; on Linux and macOS we ask to restart.
        update.download_and_install(|_, _| {}, || {}).await?;

        let should_restart = ask(
            &app,
            "Ready to Restart".to_string(),
            "The installation was successful, do you want to restart the application now?"
                .to_string(),
        )
        .await;
        if should_restart {
            app.restart();
        }
    }

    Ok(())
}

/// How long Quit waits for the core to stop the game servers before exiting anyway.
/// Longer than the core's own 45 s deadline for its servers to stop (saving their
/// worlds), after which it kills them, so the app doesn't exit with servers running.
const CORE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(60);

/// Ports the dashboard is served on, tried in order. The webview keeps localStorage
/// (saved cores, tokens, settings) per origin, and the port is part of the origin, so
/// the dashboard must get the same port every launch. A random port each time meant a
/// fresh, empty localStorage on every start, which always showed the first-time setup.
#[cfg_attr(dev, allow(dead_code))]
const DASHBOARD_PORTS: std::ops::RangeInclusive<u16> = 16663..=16672;

/// The first free port of [`DASHBOARD_PORTS`], or any free port if they are all taken.
#[cfg_attr(dev, allow(dead_code))]
fn dashboard_port() -> Option<u16> {
    pick_dashboard_port(
        DASHBOARD_PORTS,
        portpicker::is_free,
        portpicker::pick_unused_port,
    )
}

#[cfg_attr(dev, allow(dead_code))]
fn pick_dashboard_port(
    preferred: std::ops::RangeInclusive<u16>,
    is_free: impl Fn(u16) -> bool,
    any_free: impl FnOnce() -> Option<u16>,
) -> Option<u16> {
    preferred
        .into_iter()
        .find(|&port| is_free(port))
        .or_else(any_free)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_port_is_stable_when_free() {
        assert_eq!(
            pick_dashboard_port(DASHBOARD_PORTS, |_| true, || Some(1)),
            Some(16663)
        );
    }

    #[test]
    fn dashboard_port_skips_taken_ports() {
        assert_eq!(
            pick_dashboard_port(DASHBOARD_PORTS, |p| p >= 16665, || Some(1)),
            Some(16665)
        );
    }

    #[test]
    fn dashboard_port_falls_back_to_any_free_port() {
        assert_eq!(
            pick_dashboard_port(DASHBOARD_PORTS, |_| false, || Some(40000)),
            Some(40000)
        );
        assert_eq!(
            pick_dashboard_port(DASHBOARD_PORTS, |_| false, || None),
            None
        );
    }
}

/// Asks the core to shut down (stop servers, clean up), waits for that to finish or for
/// [`CORE_SHUTDOWN_TIMEOUT`], then exits the app.
async fn quit_app(
    app: AppHandle,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    mut core_done: watch::Receiver<bool>,
) {
    if let Some(tx) = shutdown_tx {
        // Err means the core has already stopped, which is fine.
        let _ = tx.send(());
    }
    if tokio::time::timeout(CORE_SHUTDOWN_TIMEOUT, core_done.wait_for(|done| *done))
        .await
        .is_err()
    {
        eprintln!(
            "Lodestone Core did not shut down within {}s, exiting anyway",
            CORE_SHUTDOWN_TIMEOUT.as_secs()
        );
    }
    app.exit(0);
}

/// Brings back the main window, which closing only hides (the core keeps running).
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tokio::main]
async fn main() {
    // Run Tauri's async tasks on this runtime instead of a second one it would create.
    tauri::async_runtime::set(tokio::runtime::Handle::current());

    // Several crates in the dependency graph enable different rustls crypto providers, so
    // rustls cannot pick a process-wide default on its own. Install aws-lc-rs before anything
    // touches TLS (lodestone_core, the updater). Errors only if one is already set.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let run_result = lodestone_core::run(lodestone_core::Args {
        is_cli: false,
        is_host: true,
        lodestone_path: None,
        disable_code_execution: false,
    })
    .await;

    let (core_fut, app_state, _guard, shutdown_tx);
    match run_result {
        Ok((fut, state, guard, tx)) => {
            core_fut = fut;
            app_state = state;
            _guard = guard;
            shutdown_tx = tx;
        }
        Err(e) => {
            show_startup_error(&e);
            return;
        }
    }

    let shutdown_tx = std::sync::Mutex::new(Some(shutdown_tx));
    // Set when the core has finished its shutdown sequence.
    let (core_done_tx, core_done_rx) = watch::channel(false);
    let quit_requested = Arc::new(AtomicBool::new(false));
    tokio::spawn({
        let quit_requested = quit_requested.clone();
        async move {
            core_fut.await;
            println!("Core has exited");
            let _ = core_done_tx.send(true);
            if !quit_requested.load(Ordering::SeqCst) {
                // The core stopped on its own (Ctrl+C, web server failure, ...).
                std::process::exit(128);
            }
        }
    });
    #[cfg_attr(dev, allow(unused_mut))]
    let mut context = tauri::generate_context!();

    // The single instance plugin must be registered first.
    #[cfg_attr(dev, allow(unused_mut))]
    let mut builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            // Launching the app again is how people look for its window once it is closed.
            show_main_window(app);
            if let Err(e) = app.emit("single-instance", Payload { args: argv, cwd }) {
                eprintln!("Failed to forward the second instance's arguments: {e}");
            }
        }));

    #[cfg(not(dev))]
    {
        // The localhost plugin binds this port later, so another program could take it in
        // between. The plugin only accepts a port number, so this cannot be closed here.
        let Some(port) = dashboard_port() else {
            show_startup_error(&"no free local port for the dashboard");
            return;
        };
        let url: Url = match format!("http://localhost:{}", port).parse() {
            Ok(url) => url,
            Err(e) => {
                show_startup_error(&e);
                return;
            }
        };
        // rewrite the config so the IPC is enabled on this URL
        context.config_mut().build.frontend_dist = Some(FrontendDist::Url(url.clone()));
        context.config_mut().build.dev_url = Some(url);

        builder = builder.plugin(tauri_plugin_localhost::Builder::new(port).build());
    }

    let app = builder
        // opens <a target="_blank"> http(s) links in the system browser, like v1's shell-open
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            is_setup,
            setup_owner_account,
            get_owner_jwt,
            get_first_time_setup_key,
            is_debug_build
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if let Err(e) = window.hide() {
                    eprintln!("Failed to hide the window: {e}");
                }
                api.prevent_close();
            }
        })
        .setup(move |app| {
            // On macOS the tray icon only opens this menu, so it needs a way back to the window.
            let show = MenuItem::with_id(app, "show", "Show Lodestone Host", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::new()
                .icon(tauri::include_image!("icons/icon.png"))
                .icon_as_template(true)
                .menu(&tray_menu)
                // v1 opened the menu on left click only on macOS; on Windows left click shows the window
                .show_menu_on_left_click(cfg!(target_os = "macos"))
                .on_menu_event(move |app, event| {
                    if event.id().as_ref() == "show" {
                        show_main_window(app);
                    } else if event.id().as_ref() == "quit" {
                        quit_requested.store(true, Ordering::SeqCst);
                        let tx = match shutdown_tx.lock() {
                            Ok(mut tx) => tx.take(),
                            Err(poisoned) => poisoned.into_inner().take(),
                        };
                        tauri::async_runtime::spawn(quit_app(
                            app.clone(),
                            tx,
                            core_done_rx.clone(),
                        ));
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // macOS has no window icons; its Dock icon is set on the Ready event instead.
            #[cfg(all(debug_assertions, not(target_os = "macos")))]
            if let Some(window) = app.get_webview_window("main") {
                window.set_icon(tauri::include_image!("icons/dev/icon.png"))?;
            }

            // Like v1, on Linux only an AppImage (or a dev build) can update itself.
            #[cfg(target_os = "linux")]
            let updater_enabled = cfg!(dev) || app.env().appimage.is_some();
            #[cfg(not(target_os = "linux"))]
            let updater_enabled = true;
            if updater_enabled {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = check_update_with_dialog(handle).await {
                        eprintln!("Failed to check for updates: {e}");
                    }
                });
            }

            Ok(())
        })
        .build(context);

    match app {
        Ok(app) => app.run(|_app, _event| {
            // After Tauri's own Ready handling, which sets the bundle icon in `tauri dev`.
            #[cfg(all(debug_assertions, target_os = "macos"))]
            if let tauri::RunEvent::Ready = _event {
                set_dev_dock_icon();
            }
            // Clicking the Dock icon (or opening the app again) while the window is hidden.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = _event
            {
                show_main_window(_app);
            }
        }),
        Err(e) => show_startup_error(&e),
    }
}

/// Tells the user that Lodestone Host could not start. Also printed, in case notifications are
/// unavailable.
fn show_startup_error(error: &dyn std::fmt::Display) {
    eprintln!("Lodestone Host failed to start: {error}");
    if let Err(e) = Notification::new()
        .summary("Lodestone Host failed to start")
        .body(&format!(
            "Oh no! Looks like Lodestone Host was unable to start. Error: {}",
            error
        ))
        .auto_icon()
        .show()
    {
        eprintln!("Failed to show notification: {e}");
    }
}
