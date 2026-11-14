use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

/// What the console says to a command typed before the core has started: the command
/// waits for it, with any others typed meanwhile, in order.
const NOT_READY_YET: &str =
    "Lodestone Core is still starting: the command runs once it has started";

/// The command console, listening on stdin from the moment the core starts running: give
/// it the core's state with [`Console::ready`] to run the commands typed.
pub struct Console {
    state: tokio::sync::oneshot::Sender<AppState>,
}

impl Console {
    /// The core has started: run the commands typed so far, then each one as it comes.
    pub fn ready(self, app_state: AppState) {
        let _ = self.state.send(app_state);
    }
}

/// Listens for commands on stdin.
///
/// Called first thing when the core runs, so stdin closing is noticed even while the core
/// is still starting. Reading stdin blocks, so it runs on its own OS thread instead of a
/// tokio worker, which it would otherwise occupy for the life of the process. The commands
/// run in a task on the runtime that called `start`, once [`Console::ready`] gave them the
/// core's state; until then they wait, and the console says so.
///
/// `on_close` is called when stdin closes (or can't be read), so the core shuts down: see
/// `RunOptions::stop_with_stdin`.
pub fn start(on_close: Option<Box<dyn FnOnce() + Send>>) -> Console {
    let (state_sender, state) = tokio::sync::oneshot::channel::<AppState>();
    let (line_sender, mut lines) = tokio::sync::mpsc::unbounded_channel::<String>();
    let ready = Arc::new(AtomicBool::new(false));
    {
        let ready = ready.clone();
        tokio::spawn(async move {
            // the core failed to start: its commands never run
            let Ok(app_state) = state.await else {
                return;
            };
            ready.store(true, Ordering::SeqCst);
            while let Some(line) = lines.recv().await {
                run_command(&line, &app_state).await;
            }
        });
    }
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
                if input.trim().is_empty() {
                    continue;
                }
                if !ready.load(Ordering::SeqCst) {
                    println!("{NOT_READY_YET}");
                }
                // fails only once the core has stopped: nothing runs commands any more
                if line_sender.send(input).is_err() {
                    debug!("The core stopped, so the command was not run");
                }
            }
        });
    if let Err(e) = spawn_result {
        warn!("Failed to start the command console: {e}");
    }
    Console {
        state: state_sender,
    }
}

async fn run_command(input: &str, app_state: &AppState) {
    let input_tokens = input.split_whitespace().collect::<Vec<&str>>();
    match input_tokens.first() {
        Some(&"di") => {
            // send input tokens without the first element
            handle_docker_command(&input_tokens[1..], app_state.clone()).await;
        }
        _ => {
            println!("Unknown command: {}", input.trim_end());
        }
    }
}

/// Calls its function when dropped: when the command console stops, however it stops.
struct CloseGuard(Option<Box<dyn FnOnce() + Send>>);

impl Drop for CloseGuard {
    fn drop(&mut self) {
        if let Some(on_close) = self.0.take() {
            on_close();
        }
    }
}
