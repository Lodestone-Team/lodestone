#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]


#[tokio::main]
async fn main() {
  tokio::spawn(async {
      lodestone_client::run().await;
  });
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

}
