#![forbid(unsafe_code)]

use clap::Parser;
use lodestone_core::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match lodestone_core::run(args).await {
        Ok((core_future, _app_state, _guard, _shutdown_tx)) => {
            core_future.await;
            // Flush the log file before exiting.
            drop(_guard);
            // Exit right away: the stdin reader thread and blocking tasks would otherwise
            // keep the runtime from shutting down.
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Lodestone Core failed to start: {e}");
            std::process::exit(1);
        }
    }
}
