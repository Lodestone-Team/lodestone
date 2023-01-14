#![forbid(unsafe_code)]
#[tokio::main]
async fn main() {
    lodestone_core::run().await.0.await.unwrap();
}
