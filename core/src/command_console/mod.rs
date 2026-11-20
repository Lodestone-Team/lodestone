use tracing::{debug, warn};

/// Listens on stdin, so that the core notices it closing.
///
/// Called first thing when the core runs, so stdin closing is noticed even while the core
/// is still starting. Reading stdin blocks, so it runs on its own OS thread instead of a
/// tokio worker, which it would otherwise occupy for the life of the process. The console
/// has no commands: a line typed in it is answered as an unknown command.
///
/// `on_close` is called when stdin closes (or can't be read), so the core shuts down: see
/// `RunOptions::stop_with_stdin`.
pub fn start(on_close: Option<Box<dyn FnOnce() + Send>>) {
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
                let input = input.trim();
                if !input.is_empty() {
                    println!("Unknown command: {input}");
                }
            }
        });
    if let Err(e) = spawn_result {
        warn!("Failed to start the command console: {e}");
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
