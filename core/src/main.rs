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

/// The most async worker threads the core runs. It mostly waits on the network, the disk and
/// its servers' processes, and CPU-heavy work goes to tokio's blocking pool, so tokio's
/// default of one worker per CPU core (16 or more on many machines) only adds idle threads.
/// `TOKIO_WORKER_THREADS` still sets the number.
const MAX_WORKER_THREADS: usize = 4;

fn main() {
    let mut runtime = tokio::runtime::Builder::new_multi_thread();
    runtime.enable_all();
    if std::env::var_os("TOKIO_WORKER_THREADS").is_none() {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        runtime.worker_threads(cpus.min(MAX_WORKER_THREADS));
    }
    runtime
        .build()
        .expect("Failed to start the async runtime")
        .block_on(run());
}

async fn run() {
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
