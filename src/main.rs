#[tokio::main]
async fn main() {
    lodestone_client::run().await.0.await.unwrap();
}
