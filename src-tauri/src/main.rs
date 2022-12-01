#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use lodestone_client::AppState;

use lodestone_client::Error;

use lodestone_client::tauri_export::is_owner_account_present;

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
    lodestone_client::tauri_export::setup_owner_account(state.inner(), username, password).await
}

#[tauri::command]
async fn get_owner_jwt(state: tauri::State<'_, AppState>) -> Result<String, ()> {
    lodestone_client::tauri_export::get_owner_jwt(state.inner())
        .await
        .ok_or(())
}

#[tokio::main]
async fn main() {
    let app_state = lodestone_client::run().await;
    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            is_setup,
            setup_owner_account,
            get_owner_jwt
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
