use tracing::{debug, warn};

use crate::AppState;

async fn handle_docker_command(input_tokens: &[&str], app_state: AppState) {
    match input_tokens.first() {
        Some(&"add") => {
            if let Some(container_name) = input_tokens.get(1) {
                app_state
                    .docker_bridge
                    .add_to_watch_list(container_name.to_string())
                    .await;
                println!("Added {} to watch list", container_name);
            } else {
                println!("Please provide a container name to add");
            }
        }
        Some(&"remove") => {
            if let Some(container_name) = input_tokens.get(1) {
                app_state
                    .docker_bridge
                    .remove_from_watch_list(container_name.to_string())
                    .await;
                println!("Removed {} from watch list", container_name);
            } else {
                println!("Please provide a container name to remove");
            }
        }
        _ => {
            println!("Unknown command");
        }
    }
}

/// Listens for commands on stdin.
///
/// Reading stdin blocks, so it runs on its own OS thread instead of a tokio worker, which
/// it would otherwise occupy for the life of the process. Commands run on the runtime that
/// called `init`.
///
/// With `on_close`, it is sent when stdin closes (or can't be read), so the core shuts
/// down: see `RunOptions::stop_with_stdin`.
pub fn init(app_state: AppState, on_close: Option<tokio::sync::oneshot::Sender<()>>) {
    let runtime = tokio::runtime::Handle::current();
    let spawn_result = std::thread::Builder::new()
        .name("lodestone-console".to_string())
        .spawn(move || {
            let stdin = std::io::stdin();
            let _on_close = CloseGuard(on_close);
            loop {
                let mut input = String::new();
                match stdin.read_line(&mut input) {
                    Ok(0) => {
                        debug!("stdin closed, command console stopped");
                        break;
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                        println!("Ignoring input that is not valid UTF-8");
                        continue;
                    }
                    Err(e) => {
                        warn!("Failed to read stdin, command console stopped: {e}");
                        break;
                    }
                }
                let input_tokens = input.split_whitespace().collect::<Vec<&str>>();
                match input_tokens.first() {
                    Some(&"di") => {
                        // send input tokens without the first element
                        runtime
                            .block_on(handle_docker_command(&input_tokens[1..], app_state.clone()));
                    }
                    _ => {
                        println!("Unknown command: {}", input);
                    }
                }
            }
        });
    if let Err(e) = spawn_result {
        warn!("Failed to start the command console: {e}");
    }
}

/// Sends its signal when dropped: when the command console stops, however it stops.
struct CloseGuard(Option<tokio::sync::oneshot::Sender<()>>);

impl Drop for CloseGuard {
    fn drop(&mut self) {
        if let Some(on_close) = self.0.take() {
            let _ = on_close.send(());
        }
    }
}
