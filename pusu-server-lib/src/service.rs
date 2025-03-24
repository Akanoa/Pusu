//! # Service Module
//!
//! This module provides the `Service` struct, which facilitates managing client interactions
//! with communication channels. It handles subscribing, unsubscribing, publishing, and consuming
//! messages for clients connected to the system. The `ChannelRegistry` serves as the central
//! registry to manage active channels and subscriptions.
//!
//! ## Overview
//!
//! The `Service` is responsible for processing requests sent by the client. Each `Service` instance
//! is tied to a unique `client_id` to identify the client using the service. Communication is managed
//! through the `Duplex` abstraction, which allows for sending and receiving data streams.
//!
//! ## Key Components
//!
//! - **ChannelRegistry**: Central registry for managing communication channels and associated clients.
//! - **Subscriptions**: Keeps track of the channels a client is subscribed to.
//! - **Client ID**: Unique identifier (based on `Ulid`) assigned to each client for identification.
//!
//! ## Functionality
//!
//! The `Service` processes requests through its `run` method, including:
//! - Subscribing to channels
//! - Unsubscribing from channels
//! - Publishing messages
//! - Consuming messages
//! - Terminating the session
//!
//! The module also provides methods for directly interacting with channels via the `ChannelRegistry`:
//! - `subscribe_to_channel`
//! - `unsubscribe_from_channel`
//! - `publish_message`
//! - `consume_message`
//!
//! ## Logging
//!
//! This module integrates with `tracing` for structured logging of client actions. The log records
//! include details about subscribing, unsubscribing, publishing, and consuming messages. Both `info`
//! and `debug` log levels are utilized for capturing detailed events.

use crate::biscuit::authorize;
use crate::channel::ChannelRegistry;
use crate::errors::PusuServerLibError;
use crate::storage::Storage;
use actix_service::{ServiceFactory, fn_service};
use biscuit_auth::PublicKey;
use prost::Message;
use pusu_protocol::pusu::{
    AuthRequest, ConsumeRequest, PublishRequest, Request, SubscribeRequest, UnsubscribeRequest,
};
use pusu_protocol::response::{
    create_auth_response, create_fail_response, create_message_response, create_ok_response,
};
use std::io::Write;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, info, trace};
use ulid::Ulid;

const DEFAULT_BUFFER_SIZE: usize = 1024;

/// Creates a service factory that builds an asynchronous service for handling
/// incoming TCP connections. Each connection is associated with a unique
/// identifier (`Ulid`) and processed using the provided `Storage` and
/// `PublicKey`.
///
/// The service handles incoming streams, creating a new `Service` instance
/// for each connection and passing the stream and `Storage` to it for
/// further processing. Errors encountered during the service runtime are
/// logged appropriately.
///
/// # Parameters
///
/// - `storage`: An instance of `Storage` that manages data persistence and
///   retrieval for the server.
/// - `public_key`: A `PublicKey` used for authentication or encryption in the
///   service.
///
/// # Returns
///
/// An implementation of `ServiceFactory` that creates services to handle
/// `TcpStream` connections.
pub fn create_service(
    storage: Storage,
    public_key: PublicKey,
) -> impl ServiceFactory<TcpStream, Error = PusuServerLibError, Response = (), InitError = (), Config = ()>
{
    fn_service(move |stream: TcpStream| {
        let storage = storage.clone();

        async move {
            let connection_id = Ulid::new();
            let mut service = Service::new(connection_id, public_key);
            if let Err(err) = service.run(stream, storage).await {
                error!(%connection_id, ?err, "An error occurred on the service")
            }

            Ok::<_, PusuServerLibError>(())
        }
    })
}

/// Represents a service responsible for managing client interactions with channels.
///
/// The `Service` struct handles subscribing to, unsubscribing from, publishing to, and consuming
/// messages from channels. It maintains a unique `client_id` for the client that owns this service
/// and keeps track of the channels the client has subscribed to. Communication is facilitated
/// through the `ChannelRegistry` object, which provides access to the underlying system of
/// channels.
///
/// # Fields
///
/// * `channel_registry` - A shared registry that manages channels and the clients subscribed to them.
/// * `subscriptions` - A list of channels to which the client is currently subscribed.
/// * `client_id` - The unique identifier of the client owning this service.
pub struct Service {
    channel_registry: Option<ChannelRegistry>,
    subscriptions: Vec<String>,
    client_id: Ulid,
    public_key: PublicKey,
}

impl Service {
    pub fn new(client_id: Ulid, public_key: PublicKey) -> Self {
        Self {
            public_key,
            channel_registry: None,
            client_id,
            subscriptions: Vec::new(),
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        info!("Service dropped...");
    }
}

impl Service {
    /// Runs the service to process client requests over a TCP connection.
    ///
    /// This method continuously reads requests from the provided `TcpStream`,
    /// decodes them, and delegates their execution to the `execute` method.
    /// Responses are then written back to the client using the same stream.
    ///
    /// # Parameters
    ///
    /// - `stream`: The TCP stream used for communication with the client.
    /// - `storage`: A `Storage` instance providing access to data storage operations.
    ///
    /// # Details
    ///
    /// - The method starts by sending an authentication response to the client.
    /// - It enters a loop where it reads incoming requests, decodes them, processes them,
    ///   and writes back the response.
    /// - If the client disconnects or an invalid request is received, the loop is terminated.
    ///
    /// # Error Handling
    ///
    /// - If a request cannot be decoded, a failure response is sent back to the client.
    /// - If writing responses or other operations fail, the method returns an error.
    ///
    /// # Logging
    ///
    /// - Logs the start of the service using the client's unique `client_id`.
    /// - Logs when a client disconnects or errors occur during request handling.
    /// - Logs when the service is stopped.
    ///
    /// # Returns
    ///
    /// - A `Result` indicating success or an error if communication or request handling fails.
    pub async fn run(
        &mut self,
        mut stream: impl AsyncReadExt + AsyncWriteExt + Unpin,
        storage: Storage,
    ) -> crate::errors::Result<()> {
        info!(client_id=%self.client_id, "Service running...");
        stream.write_all(&create_auth_response()?).await?;
        debug!(client_id=%self.client_id, "Authenticated client");
        let mut buffer = oval::Buffer::with_capacity(DEFAULT_BUFFER_SIZE);
        loop {
            let mut tmp = vec![0; DEFAULT_BUFFER_SIZE];
            if let Ok(size) = stream.read(&mut tmp).await {
                buffer.write_all(&tmp[..size])?;
                // If no data is received from the client (`size == 0`) and no additional data is
                // available in the buffer, this indicates that the client has disconnected.
                if size == 0 && buffer.available_data() == 0 {
                    info!(client_id=%self.client_id, "Client disconnected");
                    self.unsubscribe_from_all_channels().await?;
                    break;
                }

                let request = Request::decode(buffer.data());

                let request = match request {
                    Ok(request) => request,
                    Err(err) => {
                        error!(?err, "Failed to decode request",);
                        stream
                            .write_all(&create_fail_response("failed to decode request")?)
                            .await?;
                        continue;
                    }
                };

                buffer.consume(request.encoded_len());

                let request = match request.request {
                    None => {
                        stream
                            .write_all(&create_fail_response("missing request body")?)
                            .await?;
                        continue;
                    }
                    Some(request) => request,
                };

                let response = self.execute(request, storage.clone()).await?;
                stream.write_all(&response).await?;
            }
        }
        info!(client_id=%self.client_id, "Service stopped");
        Ok(())
    }

    /// Runs the service to handle client requests.
    ///
    /// This method processes incoming requests and manages their execution. The service will
    /// continue running, handling various types of requests, until a `Quit` request is received
    /// or an error occurs.
    ///
    /// # Parameters
    ///
    /// * `request` - A specific request from the client to process.
    /// * `storage` - A `Storage` instance providing access to data storage operations.
    ///
    /// # Request Handling
    ///
    /// - **Auth**: Authenticates the client using a provided Biscuit token. This includes
    ///   setting up a `ChannelRegistry` for the authenticated tenant.
    ///
    /// - **Subscribe**: Subscribes the client to a channel. Keeps track of the subscription
    ///   in the `subscriptions` list and registers it in the `ChannelRegistry`.
    ///
    /// - **Unsubscribe**: Unsubscribes the client from a channel. Removes the subscription
    ///   from the `subscriptions` list and the `ChannelRegistry`.
    ///
    /// - **Publish**: Publishes a message to a specified channel. Utilizes the `ChannelRegistry`
    ///   to add the message to the channel's queue.
    ///
    /// - **Consume**: Consumes a message from a specified channel for the client. Retrieves
    ///   the next message in the channel's queue if available.
    ///
    /// - **Quit**: Handles client disconnection by unsubscribing the client from all subscribed
    ///   channels.
    ///
    /// For each received request, this method sends an appropriate response back to the client.
    ///
    /// # Logging
    ///
    /// - Logs client actions such as authenticating, subscribing, unsubscribing, publishing,
    ///   consuming messages, and quitting.
    /// - Detailed logs are provided on errors and when specific operations fail.
    ///
    /// # Error Handling
    ///
    /// - Returns meaningful error responses if operations (authentication, subscription, etc.) fail.
    /// - Properly handles unexpected scenarios such as missing or invalid data in requests.
    pub async fn execute(
        &mut self,
        request: pusu_protocol::pusu::request::Request,
        storage: Storage,
    ) -> crate::errors::Result<Vec<u8>> {
        let response = match request {
            pusu_protocol::pusu::request::Request::Auth(AuthRequest {
                biscuit: biscuit_base_64,
            }) => {
                self.unsubscribe_from_all_channels().await?;
                match authorize(&biscuit_base_64, &self.public_key) {
                    Ok(tenant) => {
                        let channel_registry = ChannelRegistry::new(storage, &tenant);
                        self.channel_registry = Some(channel_registry);
                        debug!(client_id=%self.client_id, tenant=tenant, "Authenticated client");
                        create_ok_response()
                    }
                    Err(e) => {
                        debug!(client_id=%self.client_id, error=%e, "Failed to authorize client");
                        create_fail_response("Unable to  authenticate")
                    }
                }
            }
            pusu_protocol::pusu::request::Request::Subscribe(SubscribeRequest {
                channel: channel_name,
            }) => {
                info!(client_id=%self.client_id, channel=channel_name, "Received subscribe request");
                self.subscriptions.push(channel_name.clone());
                match self
                    .subscribe_to_channel(self.client_id, channel_name.clone())
                    .await
                {
                    Ok(_) => {
                        debug!(client_id=%self.client_id, channel=channel_name, "Subscribed to channel");
                        create_ok_response()
                    }
                    Err(e) => {
                        debug!(client_id=%self.client_id, channel=channel_name, error=%e, "Failed to subscribe to channel");
                        create_fail_response(&e.to_string())
                    }
                }
            }
            pusu_protocol::pusu::request::Request::Unsubscribe(UnsubscribeRequest {
                channel: channel_name,
            }) => {
                info!(client_id=%self.client_id, channel=channel_name, "Received unsubscribe request");
                self.subscriptions.retain(|c| c != &channel_name);

                match self
                    .unsubscribe_from_channel(self.client_id, channel_name.clone())
                    .await
                {
                    Ok(_) => {
                        debug!(client_id=%self.client_id, channel=channel_name, "Unsubscribed from channel");
                        create_ok_response()
                    }
                    Err(e) => {
                        debug!(client_id=%self.client_id, channel=channel_name, error=%e, "Failed to unsubscribe from channel");
                        create_fail_response(&e.to_string())
                    }
                }
            }
            pusu_protocol::pusu::request::Request::Publish(PublishRequest {
                channel: channel_name,
                message,
            }) => {
                let message_debug = String::from_utf8_lossy(&message).to_string();
                info!(client_id=%self.client_id, channel=channel_name, message=message_debug, "Received publish request");

                match self
                    .publish_message(channel_name.clone(), message.clone())
                    .await
                {
                    Ok(_) => {
                        debug!(client_id=%self.client_id, channel=channel_name, message=message_debug, "Message published");
                        create_ok_response()
                    }
                    Err(e) => {
                        debug!(client_id=%self.client_id, channel=channel_name, message=message_debug, error=%e, "Failed to publish message");
                        create_fail_response(&e.to_string())
                    }
                }
            }
            pusu_protocol::pusu::request::Request::Consume(ConsumeRequest {
                channel: channel_name,
            }) => {
                trace!(client_id=%self.client_id, channel=channel_name, "Received consume request");

                match self
                    .consume_message(channel_name.clone(), self.client_id)
                    .await
                {
                    Ok(maybe_message) => {
                        match &maybe_message {
                            Some(message) => {
                                let message_debug = String::from_utf8_lossy(message).to_string();
                                debug!(client_id=%self.client_id, channel=channel_name, message=message_debug, "Message found");
                            }
                            None => {
                                trace!(client_id=%self.client_id, channel=channel_name, "No message to consume");
                            }
                        }
                        create_message_response(maybe_message)
                    }
                    Err(err) => {
                        debug!(client_id=%self.client_id, channel=channel_name, error=%err, "Failed to consume message");
                        create_fail_response(&err.to_string())
                    }
                }
            }
            pusu_protocol::pusu::request::Request::Quit(_) => {
                info!(client_id=%self.client_id, "Received quit request");
                self.unsubscribe_from_all_channels().await?;
                create_ok_response()
            }
        };
        response.map_err(PusuServerLibError::from)
    }

    async fn unsubscribe_from_all_channels(&mut self) -> crate::errors::Result<()> {
        for channel in self.subscriptions.iter() {
            match self
                .unsubscribe_from_channel(self.client_id, channel.clone())
                .await
            {
                Ok(_) => {
                    debug!(client_id=%self.client_id, channel=channel, "Unsubscribed from channel");
                }
                Err(e) => {
                    debug!(client_id=%self.client_id, channel=channel, error=%e, "Failed to unsubscribe from channel");
                }
            }
        }
        Ok(())
    }
}

impl Service {
    /// Subscribes a subscriber to the specified channel.
    ///
    /// This method uses the `ChannelRegistry` to add a subscriber to a channel, creating the channel
    /// if it does not exist.
    ///
    /// # Parameters
    ///
    /// * `subscriber_id` - The unique identifier of the subscriber.
    /// * `channel_name` - The name of the channel to subscribe to.
    pub async fn subscribe_to_channel(
        &self,
        subscriber_id: Ulid,
        channel_name: String,
    ) -> crate::errors::Result<()> {
        self.channel_registry
            .as_ref()
            .ok_or(PusuServerLibError::NotAuthenticated)?
            .subscribe_channel(channel_name, subscriber_id)
            .await?;
        Ok(())
    }

    /// Unsubscribes a subscriber from the specified channel.
    ///
    /// This method uses the `ChannelRegistry` to remove a subscriber from a channel. If the channel
    /// becomes empty, it will be removed.
    ///
    /// # Parameters
    ///
    /// * `subscriber_id` - The unique identifier of the subscriber.
    /// * `channel_name` - The name of the channel to unsubscribe from.
    pub async fn unsubscribe_from_channel(
        &self,
        subscriber_id: Ulid,
        channel_name: String,
    ) -> crate::errors::Result<()> {
        self.channel_registry
            .as_ref()
            .ok_or(PusuServerLibError::NotAuthenticated)?
            .unsubscribe_channel(channel_name, subscriber_id)
            .await?;
        Ok(())
    }

    /// Publishes a message to the specified channel.
    ///
    /// This method uses the `ChannelRegistry` to add a message to the channel's queue.
    /// If the channel does not exist, the message will not be added.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel where the message will be published.
    /// * `message` - The content of the message to be published.
    pub async fn publish_message(
        &self,
        channel_name: String,
        message: Vec<u8>,
    ) -> crate::errors::Result<()> {
        self.channel_registry
            .as_ref()
            .ok_or(PusuServerLibError::NotAuthenticated)?
            .push_message(channel_name, message)
            .await?;
        Ok(())
    }

    /// Consumes a message from the specified channel for a subscriber.
    ///
    /// This method uses the `ChannelRegistry` to retrieve a message from the channel's queue for a
    /// given subscriber. A message is considered consumed only when all subscribers of the channel
    /// have indicated that they have received it. If all subscribers have consumed the message, it
    /// will be removed from the queue. Otherwise, the same message remains available for other
    /// subscribers.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel from which the message will be consumed.
    /// * `subscriber_id` - The unique identifier of the subscriber consuming the message.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// - `Ok(Some(message))` if a message exists and is retrieved from the channel's queue.
    /// - `Ok(None)` if the queue is empty or the channel does not exist.
    /// - `Err(PlushyError)` if the operation fails, such as when the service is not authenticated.
    pub async fn consume_message(
        &self,
        channel_name: String,
        subscriber_id: Ulid,
    ) -> crate::errors::Result<Option<Vec<u8>>> {
        let channel_registry = self
            .channel_registry
            .as_ref()
            .ok_or(PusuServerLibError::NotAuthenticated)?;
        channel_registry
            .consume_message(channel_name, subscriber_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::Storage;
    use fdb_testcontainer::get_db_once;
    use prost::Message;
    use pusu_protocol::pusu::{
        AuthRequest, ConsumeRequest, MessageResponse, OkResponse, PublishRequest, QuitRequest,
        SubscribeRequest, UnsubscribeRequest,
    };
    use pusu_toolbox::create_biscuit;
    use ulid::Ulid;

    #[tokio::test]
    async fn test_server() {
        tracing_subscriber::fmt::init();
        let database = get_db_once().await;
        let storage = Storage::new(database.clone());
        let (biscuit, keypair) = create_biscuit("tenant1").expect("Failed to create biscuit");

        let mut service1 = crate::service::Service::new(Ulid::new(), keypair.public());
        let mut service2 = crate::service::Service::new(Ulid::new(), keypair.public());

        // define parameters
        let channel_name = "channel";
        let message1 = b"message1";
        let message2 = b"message2";

        // authenticate to server 1
        let auth_request = pusu_protocol::pusu::request::Request::Auth(AuthRequest {
            biscuit: biscuit.to_base64().expect("Failed to encode biscuit"),
        });
        let response = service1
            .execute(auth_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Ok(OkResponse {}))
            }
        );

        // authenticate to server 2
        let auth_request = pusu_protocol::pusu::request::Request::Auth(AuthRequest {
            biscuit: biscuit.to_base64().expect("Failed to encode biscuit"),
        });
        let response = service2
            .execute(auth_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Ok(OkResponse {}))
            }
        );

        // subscribe with subscriber 1
        let subscribe_request =
            pusu_protocol::pusu::request::Request::Subscribe(SubscribeRequest {
                channel: channel_name.to_string(),
            });
        let response = service1
            .execute(subscribe_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Ok(OkResponse {}))
            }
        );

        // subscribe with subscriber 2
        let subscribe_request =
            pusu_protocol::pusu::request::Request::Subscribe(SubscribeRequest {
                channel: channel_name.to_string(),
            });
        let response = service2
            .execute(subscribe_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Ok(OkResponse {}))
            }
        );

        // publish message 1 to channel
        let publish_request = pusu_protocol::pusu::request::Request::Publish(PublishRequest {
            channel: channel_name.to_string(),
            message: message1.to_vec(),
        });
        let response = service1
            .execute(publish_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Ok(OkResponse {}))
            }
        );

        // publish message 2 to channel
        let publish_request = pusu_protocol::pusu::request::Request::Publish(PublishRequest {
            channel: channel_name.to_string(),
            message: message2.to_vec(),
        });
        let response = service1
            .execute(publish_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Ok(OkResponse {}))
            }
        );

        // consume message 1 from channel by subscriber 1
        let consume_request = pusu_protocol::pusu::request::Request::Consume(ConsumeRequest {
            channel: channel_name.to_string(),
        });
        let response = service1
            .execute(consume_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Message(
                    MessageResponse {
                        message: Some(message1.to_vec()),
                    }
                ))
            }
        );

        // try to consume message 1 from channel again by subscriber 1 but fail because subscriber 2 doesn't consume yet the
        // first message
        let consume_request = pusu_protocol::pusu::request::Request::Consume(ConsumeRequest {
            channel: channel_name.to_string(),
        });
        let response = service1
            .execute(consume_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Message(
                    MessageResponse { message: None }
                ))
            }
        );

        // consume message 1 from channel by subscriber 2
        let consume_request = pusu_protocol::pusu::request::Request::Consume(ConsumeRequest {
            channel: channel_name.to_string(),
        });
        let response = service2
            .execute(consume_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Message(
                    MessageResponse {
                        message: Some(message1.to_vec()),
                    }
                ))
            }
        );

        // unsubscribe of channel by subscriber 2
        let unsubscribe_request =
            pusu_protocol::pusu::request::Request::Unsubscribe(UnsubscribeRequest {
                channel: channel_name.to_string(),
            });
        let response = service2
            .execute(unsubscribe_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Ok(OkResponse {}))
            }
        );

        // consume message 2 from channel by subscriber 1
        let consume_request = pusu_protocol::pusu::request::Request::Consume(ConsumeRequest {
            channel: channel_name.to_string(),
        });
        let response = service1
            .execute(consume_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Message(
                    MessageResponse {
                        message: Some(message2.to_vec()),
                    }
                ))
            }
        );

        // no more message remaining in the channel for subscriber 1
        let consume_request = pusu_protocol::pusu::request::Request::Consume(ConsumeRequest {
            channel: channel_name.to_string(),
        });
        let response = service1
            .execute(consume_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Message(
                    MessageResponse { message: None }
                ))
            }
        );

        // subscriber 1 quit explicitly the server
        let quit_request = pusu_protocol::pusu::request::Request::Quit(QuitRequest {});
        let response = service1
            .execute(quit_request, storage.clone())
            .await
            .expect("Failed to execute request");
        let decoded_response = pusu_protocol::pusu::Response::decode(&response[..])
            .expect("Failed to decode response");
        assert_eq!(
            decoded_response,
            pusu_protocol::pusu::Response {
                response: Some(pusu_protocol::pusu::response::Response::Ok(OkResponse {}))
            }
        );
    }
}
