use actix_server::Server;
use clap::Parser;
use pusu_server_lib::service::create_service;
use pusu_server_lib::storage::Storage;
use std::sync::Arc;
use tracing::info;

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
            create_service(storage_for_factory, public_key)
        })?
        .run()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use pusu_protocol::request::{
        create_auth_request, create_consume_request, create_publish_request, create_quit_request,
        create_subscribe_request, create_unsubscribe_request,
    };
    use pusu_protocol::response::{create_message_response_struct, create_ok_response_struct};
    use pusu_server_lib::test_utils::Server;

    /// This is an integration test for the server's functionality.
    /// It tests the core flow of authenticating, subscribing, publishing, and consuming messages within the system.
    ///
    /// The test performs the following steps:
    /// 1. Initializes the server and related test resources.
    /// 2. Creates a Biscuit token for tenant authentication.
    /// 3. Creates three clients: `subscriber1`, `subscriber2`, and `publisher`.
    /// 4. Authenticates all three clients using the Biscuit token.
    /// 5. Subscribes both subscribers (`subscriber1` and `subscriber2`) to a specific channel.
    /// 6. Publishes multiple messages to the channel using the publisher.
    /// 7. Ensures subscribers can consume messages in the expected order:
    ///     - `subscriber1` consumes the first message successfully.
    ///     - `subscriber1` cannot consume the second message until `subscriber2` consumes the first.
    ///     - `subscriber2` consumes the first message, and the flow proceeds as expected.
    /// 8. Publishes an additional message to verify the system remains functional.
    ///
    /// This test ensures the server adheres to its behavior of broadcasting messages and gating
    /// consumption until all subscribers consume the current message.
    #[tokio::test]
    async fn test_server() {
        tracing_subscriber::fmt::init();

        let server = Server::new().await;
        let biscuit = server.get_biscuit("tenant1");
        let mut subscriber1 = server.get_client().await;
        let mut subscriber2 = server.get_client().await;
        let mut publisher = server.get_client().await;

        let channel_name = "channel";
        let message1 = b"message1";
        let message2 = b"message2";
        let message3 = b"message3";

        // Authenticate participants
        let auth_request = create_auth_request(
            &biscuit
                .to_base64()
                .expect("Unable to serialize the Biscuit"),
        )
        .expect("Unable to create the request");

        assert_eq!(
            subscriber1.call(auth_request.clone()).await,
            create_ok_response_struct()
        );
        assert_eq!(
            subscriber2.call(auth_request.clone()).await,
            create_ok_response_struct()
        );
        assert_eq!(
            publisher.call(auth_request.clone()).await,
            create_ok_response_struct()
        );

        // subscriber 1 subscribes to channel
        let subscription_request =
            create_subscribe_request(channel_name).expect("Unable to create the request");
        assert_eq!(
            subscriber1.call(subscription_request.clone()).await,
            create_ok_response_struct()
        );

        // subscriber 2 subscribes to channel
        let subscription_request =
            create_subscribe_request(channel_name).expect("Unable to create the request");
        assert_eq!(
            subscriber2.call(subscription_request.clone()).await,
            create_ok_response_struct()
        );

        // publisher publishes the message 1
        let publishing_request =
            create_publish_request(channel_name, message1).expect("Unable to create the request");
        assert_eq!(
            publisher.call(publishing_request).await,
            create_ok_response_struct()
        );

        // publisher publishes the message 2
        let publishing_request =
            create_publish_request(channel_name, message2).expect("Unable to create the request");
        assert_eq!(
            publisher.call(publishing_request).await,
            create_ok_response_struct()
        );

        // subscriber 1 consumes first message
        let consuming_request =
            create_consume_request(channel_name).expect("Unable to create the request");
        assert_eq!(
            subscriber1.call(consuming_request).await,
            create_message_response_struct(Some(message1.to_vec()))
        );

        // subscriber 1 fail to consume the message 2 because subscriber 2 hasn't consumed yet
        let consuming_request =
            create_consume_request(channel_name).expect("Unable to create the request");
        assert_eq!(
            subscriber1.call(consuming_request).await,
            create_message_response_struct(None)
        );

        // subscriber 2 consumes first message
        let consuming_request =
            create_consume_request(channel_name).expect("Unable to create the request");
        assert_eq!(
            subscriber2.call(consuming_request).await,
            create_message_response_struct(Some(message1.to_vec()))
        );

        // publisher publishes the message 3
        let publishing_request =
            create_publish_request(channel_name, message3).expect("Unable to create the request");
        assert_eq!(
            publisher.call(publishing_request).await,
            create_ok_response_struct()
        );

        // subscriber 2 unsubscribes to channel
        let consuming_request =
            create_unsubscribe_request(channel_name).expect("Unable to create the request");
        assert_eq!(
            subscriber2.call(consuming_request).await,
            create_ok_response_struct()
        );

        // subscriber 1 consumes 2nd message
        let consuming_request =
            create_consume_request(channel_name).expect("Unable to create the request");
        assert_eq!(
            subscriber1.call(consuming_request).await,
            create_message_response_struct(Some(message2.to_vec()))
        );

        // subscriber 1 consumes 3rd message
        let consuming_request =
            create_consume_request(channel_name).expect("Unable to create the request");
        assert_eq!(
            subscriber1.call(consuming_request).await,
            create_message_response_struct(Some(message3.to_vec()))
        );

        let quit_request = create_quit_request().expect("Unable to create quit request");
        subscriber1.call(quit_request).await;
    }
}
