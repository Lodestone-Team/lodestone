#![forbid(unsafe_code)]

use clap::Parser;
use lodestone_core::{Args, RunOptions};

// The core's command line: `Args`, and how a program that runs the core as a child
// process (Lodestone Host) starts it.
/// Lodestone Core, the backend that runs your game servers.
#[derive(Debug, Parser)]
struct Cli {
    #[command(flatten)]
    args: Args,
    /// Serve on exactly this port, and fail when it is taken.
    #[arg(long)]
    port: Option<u16>,
    /// Shut down, stopping the servers, when stdin closes.
    #[arg(long, default_value = "false")]
    stop_with_stdin: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let options = RunOptions {
        port: cli.port,
        stop_with_stdin: cli.stop_with_stdin,
    };
    match lodestone_core::run_with(cli.args, options).await {
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
