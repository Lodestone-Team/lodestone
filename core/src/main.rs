#![forbid(unsafe_code)]

use clap::Parser;
use lodestone_core::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    lodestone_core::run(args).await.0.await;
}
