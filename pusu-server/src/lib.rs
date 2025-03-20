use crate::errors::PusuServerError;
use actix_server::Server;
use actix_service::fn_service;
use clap::Parser;
use pusu_server_lib::service::Service;
use pusu_server_lib::storage::Storage;
use std::sync::Arc;
use tokio::net::TcpStream;
use tracing::{error, info};
use ulid::Ulid;

mod cli;
mod config;
mod errors;

pub async fn run() -> errors::Result<()> {
    let cli = cli::Cli::parse();
    let config = config::PusuServerConfig::new(&cli.configuration)?;

    let cluster_file = config.cluster_file.clone();

    // The FoundationDB API requires unsafe initialization through its C FFI
    // This is safe because:
    // 1. We're following the documented API pattern
    // 2. The _guard ensures the API stays alive for the duration of the program
    let _guard = unsafe {
        let api_server = foundationdb::api::FdbApiBuilder::default()
            .set_runtime_version(config.api_version)
            .build()?;
        api_server.boot()
    };

    let database = foundationdb::Database::new(Some(cluster_file.as_ref()))?;
    let database = Arc::new(database);
    let storage = Storage::new(database);

    let public_key = biscuit_auth::PublicKey::from_bytes_hex(&config.public_key)?;

    let bind_addr = (config.host.as_str(), config.port);

    info!(
        host = bind_addr.0,
        port = bind_addr.1,
        "Starting Puṣū server"
    );

    Server::build()
        .bind("pusu-server", bind_addr, move || {
            let storage_for_factory = storage.clone();

            fn_service(move |stream: TcpStream| {
                let storage = storage_for_factory.clone();

                async move {
                    let connection_id = Ulid::new();
                    let mut service = Service::new(connection_id, public_key);
                    if let Err(err) = service.run(stream, storage).await {
                        error!(%connection_id, ?err, "An error occurred on the service")
                    }

                    Ok::<_, PusuServerError>(())
                }
            })
        })?
        .run()
        .await?;

    Ok(())
}
