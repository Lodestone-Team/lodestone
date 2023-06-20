#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use lodestone_core::AppState;

use lodestone_core::error::Error;

use lodestone_core::auth::jwt_token::JwtToken;
use lodestone_core::tauri_export::is_owner_account_present;

use tauri::{utils::config::AppUrl, WindowUrl};
use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayEvent};
use tauri::Manager;

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

#[tokio::main]
async fn main() {
    let (core_fut, app_state, _guard) = lodestone_core::run(lodestone_core::Args {
        is_cli: false,
        is_desktop: true,
        lodestone_path: None,
    })
    .await;
    tokio::spawn(async {
        core_fut.await;
        println!("Core has exited");
        std::process::exit(128);
    });
    let mut context = tauri::generate_context!();
    let mut builder = tauri::Builder::default();

    #[cfg(not(dev))]
    {
        let port = portpicker::pick_unused_port().expect("Failed to pick unused port");
        let url = format!("http://localhost:{}", port).parse().unwrap();
        let window_url = WindowUrl::External(url);
        // rewrite the config so the IPC is enabled on this URL
        context.config_mut().build.dist_dir = AppUrl::Url(window_url.clone());
        context.config_mut().build.dev_path = AppUrl::Url(window_url);

        builder = builder.plugin(tauri_plugin_localhost::Builder::new(port).build());
    }

    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    let tray_menu = SystemTrayMenu::new()
        .add_item(quit);


    builder
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            is_setup,
            setup_owner_account,
            get_owner_jwt,
            get_first_time_setup_key
        ])
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                event.window().hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .system_tray(SystemTray::new().with_menu(tray_menu))
        .on_system_tray_event(|app, event| {
            match event {
                SystemTrayEvent::MenuItemClick { id, .. } => {
                    if id == "quit" {
                        app.exit(0);
                    }
                }

                SystemTrayEvent::LeftClick {
                    ..
                } => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                _ => {}
            }
        })
        .run(context)
        .expect("error while running tauri application");
}
