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
use crate::errors::PusuServerError;
use crate::server::Duplex;
use crate::storage::Storage;
use biscuit_auth::PublicKey;
use prost::Message;
use pusu_protocol::pusu::{
    AuthRequest, ConsumeRequest, PublishRequest, Request, SubscribeRequest, UnsubscribeRequest,
};
use pusu_protocol::response::{
    create_auth_response, create_fail_response, create_message_response, create_ok_response,
};
use tracing::{debug, error, info};
use ulid::Ulid;

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
        println!("Service dropped...");
    }
}

impl Service {
    /// Runs the service to handle client requests.
    ///
    /// This method processes incoming requests through the given `Duplex` object. The service will
    /// continue running, handling various types of requests, until a `Quit` request is received.
    ///
    /// # Parameters
    ///
    /// * `duplex` - A `Duplex` struct containing read and write channels for communication.
    ///
    /// # Request Handling
    ///
    /// - **Subscribe**: Subscribes the client to a channel. Stores the subscription in the `subscriptions`
    ///   list and registers it in the `ChannelRegistry`.
    ///
    /// - **Unsubscribe**: Unsubscribes the client from a channel. Removes the subscription both internally
    ///   and from the `ChannelRegistry`.
    ///
    /// - **Publish**: Publishes a message to a specified channel. Utilizes the `ChannelRegistry` to add
    ///   the message to the channel's queue.
    ///
    /// - **Consume**: Consumes a message from a specified channel for the client. Retrieves the next message
    ///   in the channel's queue, if available.
    ///
    /// - **Quit**: Handles client disconnection. Unsubscribes the client from all subscribed channels and
    ///   terminates the service.
    ///
    /// For each received request, this method sends an appropriate response through the `duplex`'s write channel.
    ///
    /// # Logging
    ///
    /// - Logs client actions such as subscribing, unsubscribing, publishing, consuming, and quitting.
    /// - Debug logs are used for more detailed actions (e.g., unsubscribing and message handling).
    ///
    /// # Panics
    ///
    /// * This method may panic if sending responses through the write channel fails unexpectedly.
    pub async fn run(
        &mut self,
        mut duplex: Duplex<Vec<u8>, Vec<u8>>,
        storage: Storage,
    ) -> crate::errors::Result<()> {
        info!(client_id=%self.client_id, "Service running...");
        duplex.write(create_auth_response()?).await?;
        loop {
            if let Ok(request) = duplex.read().await {
                let request = Request::decode(&*request);

                let request = match request {
                    Ok(request) => request,
                    Err(err) => {
                        error!(?err, "Failed to decode request",);
                        duplex
                            .write(create_fail_response("failed to decode request")?)
                            .await?;
                        continue;
                    }
                };

                let request = match request.request {
                    None => {
                        duplex
                            .write(create_fail_response("missing request body")?)
                            .await?;
                        continue;
                    }
                    Some(request) => request,
                };

                match request {
                    pusu_protocol::pusu::request::Request::Auth(AuthRequest {
                        biscuit: biscuit_base_64,
                    }) => match authorize(&biscuit_base_64, &self.public_key) {
                        Ok(tenant) => {
                            let channel_registry = ChannelRegistry::new(storage.clone(), &tenant);
                            self.channel_registry = Some(channel_registry);
                            debug!(client_id=%self.client_id, tenant=tenant, "Authenticated client");
                            duplex.write(create_ok_response()?).await?;
                        }
                        Err(e) => {
                            debug!(client_id=%self.client_id, error=%e, "Failed to authorize client");
                            duplex
                                .write(create_fail_response("Unable to  authenticate")?)
                                .await?;
                        }
                    },
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
                                duplex.write(create_ok_response()?).await?;
                            }
                            Err(e) => {
                                debug!(client_id=%self.client_id, channel=channel_name, error=%e, "Failed to subscribe to channel");
                                duplex.write(create_fail_response(&e.to_string())?).await?;
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
                                duplex.write(create_ok_response()?).await?;
                            }
                            Err(e) => {
                                debug!(client_id=%self.client_id, channel=channel_name, error=%e, "Failed to unsubscribe from channel");
                                duplex.write(create_fail_response(&e.to_string())?).await?;
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
                                duplex.write(create_ok_response()?).await?;
                            }
                            Err(e) => {
                                debug!(client_id=%self.client_id, channel=channel_name, message=message_debug, error=%e, "Failed to publish message");
                                duplex.write(create_fail_response(&e.to_string())?).await?;
                            }
                        }
                    }
                    pusu_protocol::pusu::request::Request::Consume(ConsumeRequest {
                        channel: channel_name,
                    }) => {
                        info!(client_id=%self.client_id, channel=channel_name, "Received consume request");

                        match self
                            .consume_message(channel_name.clone(), self.client_id)
                            .await
                        {
                            Ok(maybe_message) => {
                                match &maybe_message {
                                    Some(message) => {
                                        let message_debug =
                                            String::from_utf8_lossy(message).to_string();
                                        debug!(client_id=%self.client_id, channel=channel_name, message=message_debug, "Message found");
                                    }
                                    None => {
                                        debug!(client_id=%self.client_id, channel=channel_name, "No message to consume");
                                    }
                                }
                                duplex
                                    .write(create_message_response(maybe_message)?)
                                    .await?
                            }
                            Err(err) => {
                                debug!(client_id=%self.client_id, channel=channel_name, error=%err, "Failed to consume message");
                                duplex
                                    .write(create_fail_response(&err.to_string())?)
                                    .await?;
                            }
                        }
                    }
                    pusu_protocol::pusu::request::Request::Quit(_) => {
                        info!(client_id=%self.client_id, "Received quit request");
                        for channel in self.subscriptions.iter() {
                            match self
                                .unsubscribe_from_channel(self.client_id.clone(), channel.clone())
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
                        duplex.write(create_ok_response()?).await?;
                        break;
                    }
                }
            }
        }
        info!(client_id=%self.client_id, "Service stopped");
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
            .ok_or(PusuServerError::NotAuthenticated)?
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
            .ok_or(PusuServerError::NotAuthenticated)?
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
            .ok_or(PusuServerError::NotAuthenticated)?
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
            .ok_or(PusuServerError::NotAuthenticated)?;
        channel_registry
            .consume_message(channel_name, subscriber_id)
            .await
    }
}
