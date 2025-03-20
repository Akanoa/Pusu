use tracing::error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    if let Err(err) = pusu_server::run().await {
        error!(%err, "Pusu server error");
    };
}
