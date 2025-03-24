//! # Publish-Subscribe Messaging System
//!
//! This file implements a publish-subscribe messaging system. It provides
//! structures and methods to model communication channels, manage subscribers,
//! handle message queues, and track message consumption. The system ensures reliable
//! and orderly message distribution while maintaining simple management of channels
//! and subscribers.
//!
//! ## Key Components
//!
//! - **Channel Structure**: Represents an individual communication channel.
//!   - Handles a queue of messages (FIFO order).
//!   - Manages a set of active subscribers.
//!   - Tracks which subscribers have consumed specific messages.
//!
//! - **ChannelRegistry Structure**: Manages a collection of independent `Channel` instances.
//!   - Enables creating, retrieving, and deleting channels.
//!   - Allows adding messages to channel queues.
//!   - Supports subscribing and unsubscribing to specific channels.
//!   - Ensures messages are only removed after all active subscribers consume them.
//!
//! ## Usage Scenarios
//!
//! This system is ideal for:
//! - Real-time communication platforms.
//! - Notification systems requiring persistent message tracking.
//! - Reliable task or event distribution frameworks.
//!
//! ## Structure Overview
//!
//! - `Channel`:
//!   - Represents a publish-subscribe channel with a queue for messages,
//!     a list of subscribers, and mechanisms for tracking message consumption.
//!
//! - `ChannelRegistry`:
//!   - Manages multiple channels, offering APIs for adding messages, managing
//!     subscriptions, and reliably consuming messages.
//!
//! - **Example Test (`tests`)**:
//!   - Illustrates the basic behavior of the system, including channel creation,
//!     adding messages, and sequential message consumption by subscribers.
//! ---

use crate::storage::Storage;
use crate::DataPrefix;
use foundationdb::tuple::{Element, Subspace};
use prost::Message;
use pusu_protocol::errors::PusuProtocolError;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use tracing::{debug, trace};
use ulid::Ulid;

/// Converts a `u128` value into a tuple of two `u64` values.
///
/// This function splits a `u128` integer into a high-order (`msb`, most significant bits)
/// and low-order (`lsb`, least significant bits) `u64` tuple representation.
///
/// # Parameters
///
/// * `a` - A 128-bit unsigned integer to be converted.
///
/// # Returns
///
/// A tuple `(u64, u64)` containing the most significant and least significant 64 bits of the input `u128`.
fn u128_to_tuple(a: u128) -> (u64, u64) {
    ((a >> 64) as u64, a as u64)
}

impl From<crate::pusu::Channel> for Channel {
    fn from(value: crate::pusu::Channel) -> Self {
        Channel {
            name: value.name,
            queue: value.queue.into_iter().collect(),
            subscribers: value
                .subscribers
                .into_iter()
                .map(|crate::pusu::Ulid { msb, lsb }| (msb, lsb))
                .map(Ulid::from)
                .collect(),
            consumed_by_subscribers: value
                .consumed_by_subscribers
                .into_iter()
                .map(|crate::pusu::Ulid { msb, lsb }| (msb, lsb))
                .map(Ulid::from)
                .collect(),
            locked: value.locked,
        }
    }
}

/// Converts a `Ulid` instance into its Protobuf representation.
///
/// This function takes a `Ulid` and splits it into its most significant bits (MSB)
/// and least significant bits (LSB) as a tuple of two `u64` values. It then constructs
/// a `crate::pusu::Ulid` using the MSB and LSB.
///
/// # Parameters
///
/// * `ulid` - The `Ulid` instance to be converted into a Protobuf-compatible format.
///
/// # Returns
///
/// A `crate::pusu::Ulid` containing the MSB and LSB representation of the input `Ulid`.
fn to_protobuf_ulid(ulid: &Ulid) -> crate::pusu::Ulid {
    let (msb, lsb) = u128_to_tuple(ulid.0);
    crate::pusu::Ulid { msb, lsb }
}

impl From<&Channel> for crate::pusu::Channel {
    fn from(value: &Channel) -> Self {
        crate::pusu::Channel {
            name: value.name.clone(),
            queue: value.queue.iter().cloned().collect(),
            subscribers: value.subscribers.iter().map(to_protobuf_ulid).collect(),
            consumed_by_subscribers: value
                .consumed_by_subscribers
                .iter()
                .map(to_protobuf_ulid)
                .collect(),
            locked: value.locked,
        }
    }
}

/// Represents a communication channel in the publish-subscribe messaging system.
///
/// Each channel maintains a message queue, a set of subscribers, and a record of which
/// subscribers have consumed messages. This structure allows for sequential message
/// consumption while ensuring all subscribers must read a message before it is removed
/// from the queue.
#[derive(Default, Deserialize, Serialize)]
struct Channel {
    /// The name of the channel
    name: String,
    /// The queue of messages for the channel. Messages are processed in FIFO (First In, First Out) order.
    queue: VecDeque<Vec<u8>>,
    /// A set containing unique identifiers of subscribers subscribed to this channel.
    subscribers: HashSet<Ulid>,
    /// A set tracking which subscribers have consumed the current message from the queue.
    consumed_by_subscribers: HashSet<Ulid>,
    /// The channel is locked
    locked: bool,
}

impl Channel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }
}

/// A registry that manages communication channels in a publish-subscribe messaging system.
///
/// The `ChannelRegistry` structure maintains a collection of channels, where each channel operates
/// independently. Each channel includes a message queue, a set of subscribers, and a tracking
/// mechanism for message consumption by subscribers. This facilitates controlled and sequential
/// message delivery while ensuring that all subscribers have a chance to consume a message.
///
/// Key features provided by the `ChannelRegistry`:
/// - Maintain a collection of named channels.
/// - Support adding and retrieving messages from channel queues.
/// - Facilitate subscribing and unsubscribing for individual channels.
/// - Ensure messages are processed only after all subscribers have consumed them.
///
/// This structure is useful in scenarios that require reliable message distribution and tracking,
/// such as in publish-subscribe systems or message-queuing frameworks.
#[derive(Clone)]
pub struct ChannelRegistry {
    storage: Storage,
    tenant: Subspace,
}

impl ChannelRegistry {
    /// Computes the primary key for a specified channel name.
    ///
    /// This function constructs a database subspace for a channel based on its name.
    /// The primary key is used to uniquely identify the channel within the database,
    /// allowing associated data to be stored and retrieved efficiently.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel for which to compute the primary key.
    ///
    /// # Returns
    ///
    /// A `Subspace` representing the primary key for the specified channel.
    fn get_channel_pk(&self, channel_name: &str) -> Subspace {
        self.tenant
            .subspace(&DataPrefix::Channel)
            .subspace(&Element::String(Cow::from(channel_name)))
    }

    /// Retrieves a channel by its name from the database.
    ///
    /// This function attempts to load a `Channel` from the database based on the
    /// provided channel name. If the channel exists, it returns the deserialized
    /// `Channel` instance. If the channel does not exist, it returns `None`.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel to retrieve.
    ///
    /// # Returns
    ///
    /// A `Result` containing:
    /// - `Ok(Some(Channel))` if the channel exists.
    /// - `Ok(None)` if the channel does not exist.
    /// - `Err` if an error occurs during retrieval or deserialization.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an issue with database access or deserialization of the retrieved data.
    async fn get_channel(&self, channel_name: &str) -> crate::errors::Result<Option<Channel>> {
        let pk = self.get_channel_pk(channel_name).into_bytes();
        loop {
            trace!(channel = %channel_name, "Getting channel");
            return if let Some(bytes) = self.storage.get(&pk).await? {
                let channel = crate::pusu::Channel::decode(&*bytes)
                    .map_err(PusuProtocolError::DecodeError)?;

                let mut channel: Channel = channel.into();

                if channel.is_locked() {
                    continue;
                }

                debug!(channel = %channel_name, "Channel acquired");

                channel.lock();
                self.store_channel(&channel).await?;

                Ok(Some(channel))
            } else {
                Ok(None)
            };
        }
    }

    #[cfg(test)]
    async fn get_channel_no_lock(
        &self,
        channel_name: &str,
    ) -> crate::errors::Result<Option<Channel>> {
        let pk = self.get_channel_pk(channel_name).into_bytes();

        trace!(channel = %channel_name, "Getting channel");
        if let Some(bytes) = self.storage.get(&pk).await? {
            let channel =
                crate::pusu::Channel::decode(&*bytes).map_err(PusuProtocolError::DecodeError)?;

            let channel: Channel = channel.into();

            debug!(channel = %channel_name, "Channel acquired");
            self.store_channel(&channel).await?;

            Ok(Some(channel))
        } else {
            Ok(None)
        }
    }

    /// Stores a channel in the database.
    ///
    /// This function serializes the provided `Channel` instance and saves it in the database
    /// using the channel's name as a key. It ensures persistent storage of the channel's
    /// state, allowing it to be retrieved later.
    ///
    /// # Parameters
    ///
    /// * `channel` - A reference to the `Channel` instance to be stored.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization of the channel fails or if the database operation fails.
    async fn store_channel(&self, channel: &Channel) -> crate::errors::Result<()> {
        let pk = self.get_channel_pk(&channel.name).into_bytes();

        let channel: crate::pusu::Channel = channel.into();

        let mut bytes = Vec::new();
        channel
            .encode(&mut bytes)
            .map_err(PusuProtocolError::EncodeError)?;
        self.storage.set(&pk, &bytes).await?;
        Ok(())
    }

    async fn store_channel_unlocked(&self, channel: &mut Channel) -> crate::errors::Result<()> {
        channel.unlock();
        debug!(channel = %channel.name, "Channel unlocked");
        self.store_channel(channel).await
    }

    /// Deletes a channel from the database.
    ///
    /// This function removes a channel and all associated data from the database
    /// using the channel's name as the key. It ensures that the channel is no longer
    /// accessible, effectively "deleting" it. This is useful for cleanup operations
    /// or when a channel is no longer needed.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel to be deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    async fn delete_channel(&self, channel_name: &str) -> crate::errors::Result<()> {
        let pk = self.get_channel_pk(channel_name).into_bytes();
        self.storage.delete(&pk).await?;
        Ok(())
    }
}

impl ChannelRegistry {
    pub fn new(storage: Storage, tenant_id: &str) -> Self {
        let tenant = Subspace::from(tenant_id);

        Self { storage, tenant }
    }

    /// Adds a message to the specified channel's queue.
    ///
    /// This method allows pushing a new message to the queue of a given channel.
    /// If the specified channel does not exist or has no subscribers, the message
    /// will not be added, as channels without subscribers are not maintained.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel where the message will be pushed.
    /// * `message` - The message content to be added to the channel's queue.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The provided message is not valid UTF-8.
    /// - There is an issue with retrieving or storing the channel in the database.
    pub async fn push_message(
        &self,
        channel_name: String,
        message: Vec<u8>,
    ) -> crate::errors::Result<()> {
        debug!(
            channel = %channel_name,
            message = std::str::from_utf8(&message)?,
            "Pushing message to channel2");
        if let Some(mut channel) = self.get_channel(&channel_name).await? {
            let message_debug = std::str::from_utf8(&message)?;
            debug!(
                channel = channel_name,
                message = message_debug,
                "Pushing message to channel"
            );
            channel.queue.push_back(message);
            self.store_channel_unlocked(&mut channel).await?;
        }
        Ok(())
    }

    /// Subscribes a subscriber to a channel, creating the channel if it does not exist.
    ///
    /// If the specified channel does not exist, this method will create the channel and add the subscriber to it.
    /// If the channel already exists, the subscriber will be added to the set of subscribers.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel to subscribe to.
    /// * `subscriber_id` - The unique identifier of the subscriber to add.
    ///
    /// # Errors
    ///
    /// This function returns an error if the channel retrieval or storage operation fails.
    pub async fn subscribe_channel(
        &self,
        channel_name: String,
        subscriber_id: Ulid,
    ) -> crate::errors::Result<()> {
        debug!(
            channel = %channel_name,
            subscriber = %subscriber_id,
            "Subscribing to channel");
        match self.get_channel(&channel_name).await? {
            Some(mut channel) => {
                debug!("Channel acquired for subscription");
                channel.subscribers.insert(subscriber_id);
                self.store_channel_unlocked(&mut channel).await?;
            }
            None => {
                let mut channel = Channel::new(channel_name.clone());
                channel.subscribers.insert(subscriber_id);
                self.store_channel_unlocked(&mut channel).await?;
            }
        }
        Ok(())
    }

    /// Unsubscribes a subscriber from a specific channel and removes the channel if it becomes empty.
    ///
    /// This method removes the specified subscriber from the list of subscribers in the given channel.
    /// If the channel has no remaining subscribers after the removal, it will be deleted from the database.
    /// Additionally, any references to the subscriber in the message consumption tracking system will
    /// also be cleared. If there are still active subscribers, the channel state will be updated and
    /// stored in the database.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel from which the subscriber will be removed.
    /// * `subscriber_id` - The unique identifier of the subscriber to be unsubscribed.
    ///
    /// # Errors
    ///
    /// This function returns an error if the channel retrieval, update, or deletion operation fails.
    pub async fn unsubscribe_channel(
        &self,
        channel_name: String,
        subscriber_id: Ulid,
    ) -> crate::errors::Result<()> {
        debug!(
            channel = %channel_name,
            subscriber = %subscriber_id,
            "Unsubscribing from channel");
        if let Some(mut channel) = self.get_channel(&channel_name).await? {
            channel.subscribers.remove(&subscriber_id);
            channel.consumed_by_subscribers.remove(&subscriber_id);

            let _ = channel.duplicate_or_consume_message(subscriber_id);

            if channel.subscribers.is_empty() {
                self.delete_channel(&channel_name).await?;
            } else {
                self.store_channel_unlocked(&mut channel).await?;
            }
        }
        Ok(())
    }

    ///
    /// Consumes a message from the specified channel for the given subscriber.
    ///
    /// This function retrieves the next message in the channel's queue for the specified subscriber.
    /// If the message has already been consumed by the subscriber, it does nothing and returns `None`.
    /// Otherwise, the message is marked as consumed for the subscriber and processed according to the
    /// list of all subscribers' consumption statuses.
    ///
    /// If all subscribers have consumed the oldest message in the queue, the message is removed.
    /// Otherwise, the message remains available for other subscribers to consume.
    ///
    /// # Parameters
    ///
    /// * `channel_name` - The name of the channel from which the subscriber wants to consume the message.
    /// * `subscriber_id` - The unique identifier of the subscriber attempting to consume the message.
    ///
    /// # Returns
    ///
    /// Returns an `Option<Vec<u8>>` containing the message if the subscriber successfully consumes it
    /// or duplicates it. Returns `None` if no message is available or if the message has already been
    /// processed by the subscriber.
    ///
    /// # Errors
    ///
    /// Returns an error if the channel retrieval or message consumption operation fails.
    pub async fn consume_message(
        &self,
        channel_name: String,
        subscriber_id: Ulid,
    ) -> crate::errors::Result<Option<Vec<u8>>> {
        debug!(
            channel = %channel_name,
            subscriber = %subscriber_id,
            "Consuming message from channel");
        match self.get_channel(&channel_name).await? {
            Some(mut channel) => {
                if channel.consumed_by_subscribers.contains(&subscriber_id) {
                    trace!(%subscriber_id, "Already consumed by subscriber");
                    self.store_channel_unlocked(&mut channel).await?;
                    return Ok(None);
                }
                channel.consumed_by_subscribers.insert(subscriber_id);
                let result = channel.duplicate_or_consume_message(subscriber_id);
                if result.is_none() {
                    channel.consumed_by_subscribers.remove(&subscriber_id);
                }
                self.store_channel_unlocked(&mut channel).await?;
                Ok(result)
            }
            None => {
                trace!(channel = %channel_name, "Channel does not exist");
                Ok(None)
            }
        }
    }
}

impl Channel {
    /// Handles the consumption or duplication of a message for a subscriber in a given channel.
    ///
    /// This function checks if all the current subscribers have consumed the oldest message
    /// in the channel. If all subscribers have consumed the message, it is removed from the queue.
    /// Otherwise, the message will remain in the queue for other subscribers to consume.
    ///
    /// # Parameters
    ///
    /// * `channel` - A mutable reference to the channel containing the queue and subscribers.
    /// * `channel_name` - The name of the channel for logging purposes.
    /// * `subscriber_id` - The unique identifier of the subscriber performing the action.
    ///
    /// # Returns
    ///
    /// Returns an `Option<String>` containing the message if the subscriber is able to consume or duplicate it.
    /// Returns `None` if no message is available or if the message has already been processed.
    fn duplicate_or_consume_message(&mut self, subscriber_id: Ulid) -> Option<Vec<u8>> {
        debug!(%subscriber_id, "Number of subscribers {}", self.subscribers.len());
        debug!(
            %subscriber_id,
            "Number of consumed by subscribers {}",
            self.consumed_by_subscribers.len()
        );
        debug!(%subscriber_id,"Front of queue: {:?}", self.queue.front());

        if self.consumed_by_subscribers.is_superset(&self.subscribers) {
            debug!(
                channel = self.name,
                subscriber = %subscriber_id,
                "Consuming message from channel"
            );
            if let Some(message) = self.queue.pop_front() {
                self.consumed_by_subscribers.clear();
                return Some(message);
            }
            None
        } else {
            debug!(
                    channel = self.name,
                    subscriber = %subscriber_id,
                    "Message not consumed yet");

            self.queue.front().cloned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdb_testcontainer::get_db_once;
    use tracing::warn;
    use ulid::Ulid;

    /// This test verifies the correct behavior of the `Database` system regarding scenarios
    /// where a channel is not created without any subscribers.
    ///
    /// The test covers the following cases:
    ///
    /// 1. Pushing messages to a channel without any subscribers does not create the channel.
    /// 2. Adding a subscriber to a non-existent channel creates the channel.
    /// 3. Messages pushed to the channel after a subscriber is added are stored properly.
    /// 4. Ensures the system processes the sequence of events gracefully, maintaining the
    ///    channel's integrity and message order once subscribers are present.
    #[tokio::test]
    async fn test_pubsub_behavior() {
        tracing_subscriber::fmt::init();
        let database = get_db_once().await;
        let storage = Storage::new(database.clone());
        let channel_registry = ChannelRegistry::new(storage, "tenant1");

        // Step 1: Create a channel and subscribe two users
        let channel_name = "test_channel".to_string();
        let subscriber_id_1 = Ulid::new();
        let subscriber_id_2 = Ulid::new();

        // Subscribe to the channel
        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_id_1)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_id_2)
            .await
            .expect("Failed to subscribe");

        // Ensure the channel is in the database
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );

        // Step 2: Push a message to the channel
        let message = b"Hello, World!";
        channel_registry
            .push_message(channel_name.clone(), message.to_vec())
            .await
            .expect("Failed to push message");

        // Ensure the message is in the queue
        assert_eq!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .front(),
            Some(&message.to_vec())
        );

        // Step 3: Consume the message with subscriber 1
        let consumed_msg_1 = channel_registry
            .consume_message(channel_name.clone(), subscriber_id_1)
            .await
            .expect("Failed to consume message");
        assert_eq!(consumed_msg_1, Some(message.to_vec()));

        // Ensure the message is still in the queue because subscriber 2 hasn't consumed it
        assert_eq!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .front(),
            Some(&message.to_vec())
        );

        // Step 4: Consume the message with subscriber 2
        let consumed_msg_2 = channel_registry
            .consume_message(channel_name.clone(), subscriber_id_2)
            .await
            .expect("Failed to consume message");
        assert_eq!(consumed_msg_2, Some(message.to_vec()));

        // Now the message should be removed from the queue since all subscribers consumed it
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );
    }

    /// This test ensures the `ChannelRegistry` correctly handles scenarios with multiple subscribers
    /// and message queue management, covering the following behaviors:
    ///
    /// 1. Multiple subscribers subscribing to the same channel.
    /// 2. Publishing messages to a channel.
    /// 3. Verifying all subscribers can consume the published messages independently,
    ///    ensuring messages are properly retained in the queue until all subscribers
    ///    have consumed them.
    /// 4. Unsubscribing a subscriber from a channel and verifying that they can no longer
    ///    consume messages while active subscribers continue to function as intended.
    /// 5. Ensuring the message queue is emptied only after all active subscribers have fully
    ///    consumed the messages.
    ///
    /// The test validates the system's correctness under both subscriber additions
    /// and removals, ensuring consistent message delivery and queue state management.
    #[tokio::test]
    async fn test_no_channel_creation_without_subscribers() {
        let database = get_db_once().await;
        let storage = Storage::new(database.clone());
        let channel_registry = ChannelRegistry::new(storage, "tenant1");

        // Step 1: Try pushing messages to a non-existing channel
        let channel_name = "non_existent_channel".to_string();
        let message_1 = b"First message without subscribers".to_vec();
        let message_2 = b"Second message without subscribers".to_vec();

        channel_registry
            .push_message(channel_name.clone(), message_1)
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_name.clone(), message_2.clone())
            .await
            .expect("Failed to push message");

        // Ensure the channel does not exist in the database
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get channel")
                .is_none()
        );

        // Step 2: Add a subscriber to the channel
        let subscriber_id = Ulid::new();
        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_id)
            .await
            .expect("Failed to subscribe");

        // Step 3: Push more messages after subscription
        let message_3 = b"Third message after subscription".to_vec();
        let message_4 = b"Fourth message after subscription".to_vec();
        channel_registry
            .push_message(channel_name.clone(), message_3.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_name.clone(), message_4.clone())
            .await
            .expect("Failed to push message");

        // Step 4: Verify messages are consumed in order and the channel works
        assert_eq!(
            channel_registry
                .consume_message(channel_name.clone(), subscriber_id)
                .await
                .expect("Failed to consume message"),
            Some(message_3)
        );
        assert_eq!(
            channel_registry
                .consume_message(channel_name.clone(), subscriber_id)
                .await
                .expect("Failed to consume message"),
            Some(message_4)
        );
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );

        // Step 5: Add one more subscriber to the channel
        let subscriber_id_2 = Ulid::new();
        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_id_2)
            .await
            .expect("Failed to subscribe");

        // Step 6: Push additional messages to the channel
        let message_5 = b"Fifth message after adding new subscriber".to_vec();
        let message_6 = b"Sixth message after adding new subscriber".to_vec();
        channel_registry
            .push_message(channel_name.clone(), message_5.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_name.clone(), message_6.clone())
            .await
            .expect("Failed to push message");

        // Step 7: Verify messages are consumed in order by both subscribers
        // Consume by subscriber 1
        assert_eq!(
            channel_registry
                .consume_message(channel_name.clone(), subscriber_id)
                .await
                .expect("Failed to consume message"),
            Some(message_5.clone())
        );
        // Consume by subscriber 2
        assert_eq!(
            channel_registry
                .consume_message(channel_name.clone(), subscriber_id_2)
                .await
                .expect("Failed to consume message"),
            Some(message_5.clone())
        );

        // Consume by subscriber 1
        assert_eq!(
            channel_registry
                .consume_message(channel_name.clone(), subscriber_id)
                .await
                .expect("Failed to consume message"),
            Some(message_6.clone())
        );

        // Consume by subscriber 2
        assert_eq!(
            channel_registry
                .consume_message(channel_name.clone(), subscriber_id_2)
                .await
                .expect("Failed to consume message"),
            Some(message_6.clone())
        );

        // Ensure the queue is empty after all subscribers have consumed the messages
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );
    }

    ///
    /// This test verifies the behavior of the `ChannelRegistry` struct when multiple subscribers
    /// are subscribed to a channel, and some of them are unsubscribed during the test.
    /// It ensures the correctness of message delivery and queue management in the following scenarios:
    ///
    /// 1. Subscribing multiple users to the same channel.
    /// 2. Publishing messages to the channel and verifying that all active subscribers
    ///    can consume the messages.
    /// 3. Ensuring the message queue is empty after all active subscribers have consumed
    ///    the published messages.
    /// 4. Unsubscribing a subscriber from the channel.
    /// 5. Checking that unsubscribed users cannot consume messages after being removed,
    ///    while remaining subscribers continue working as expected.
    #[tokio::test]
    async fn test_three_subscribers_with_unsubscribe() {
        let database = get_db_once().await;
        let storage = Storage::new(database.clone());
        let channel_registry = ChannelRegistry::new(storage, "tenant1");

        // Step 1: Create a channel and subscribe three users
        let channel_name = "multi_subscriber_channel".to_string();
        let subscriber_id_1 = Ulid::new();
        let subscriber_id_2 = Ulid::new();
        let subscriber_id_3 = Ulid::new();

        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_id_1)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_id_2)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_id_3)
            .await
            .expect("Failed to subscribe");

        // Ensure the channel is in the database
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );

        // Step 2: Publish some messages
        let message_1 = b"Message 1".to_vec();
        let message_2 = b"Message 2".to_vec();

        channel_registry
            .push_message(channel_name.clone(), message_1.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_name.clone(), message_2.clone())
            .await
            .expect("Failed to push message");

        // Step 3: Ensure all three subscribers can consume messages
        for subscriber_id in [&subscriber_id_1, &subscriber_id_2, &subscriber_id_3] {
            // Consume message 1
            assert_eq!(
                channel_registry
                    .consume_message(channel_name.clone(), *subscriber_id)
                    .await
                    .expect("Failed to consume message"),
                Some(message_1.clone())
            );
        }

        for subscriber_id in [&subscriber_id_1, &subscriber_id_2, &subscriber_id_3] {
            // Consume message 2
            assert_eq!(
                channel_registry
                    .consume_message(channel_name.clone(), *subscriber_id)
                    .await
                    .expect("Failed to consume message"),
                Some(message_2.clone())
            );
        }

        // Ensure the queue is empty as all messages have been consumed
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );

        // Step 4: Remove one subscriber
        channel_registry
            .unsubscribe_channel(channel_name.clone(), subscriber_id_3)
            .await
            .expect("Failed to unsubscribe");

        // Step 5: Publish new messages
        let message_3 = b"Message 3".to_vec();
        let message_4 = b"Message 4".to_vec();

        channel_registry
            .push_message(channel_name.clone(), message_3.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_name.clone(), message_4.clone())
            .await
            .expect("Failed to push message");

        // Step 6: Ensure only the remaining subscribers can consume messages
        for subscriber_id in [&subscriber_id_1, &subscriber_id_2] {
            // Consume message 3
            assert_eq!(
                channel_registry
                    .consume_message(channel_name.clone(), *subscriber_id)
                    .await
                    .expect("Failed to consume message"),
                Some(message_3.clone())
            );
        }

        for subscriber_id in [&subscriber_id_1, &subscriber_id_2] {
            // Consume message 4
            assert_eq!(
                channel_registry
                    .consume_message(channel_name.clone(), *subscriber_id)
                    .await
                    .expect("Failed to consume message"),
                Some(message_4.clone())
            );
        }

        // Ensure the queue is empty as all messages have been consumed by the two remaining subscribers
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );
    }

    /// This test verifies the behavior of the `ChannelRegistry` struct when multiple channels
    /// have multiple subscribers, and some subscribers are unsubscribed during the
    /// test. It validates the following scenarios:
    ///
    /// 1. Creating multiple channels and subscribing different sets of users to each channel.
    /// 2. Publishing messages to the channels and ensuring each subscriber can consume
    ///    all the messages published to their subscribed channel.
    /// 3. Verifying that the message queue is empty after all of the subscribers have
    ///    consumed the messages.
    /// 4. Unsubscribing certain subscribers from their respective channels.
    /// 5. Ensuring that unsubscribed users cannot consume messages after they
    ///    have been removed from the channel, while remaining subscribers can still
    ///    consume the new messages.
    #[tokio::test]
    async fn test_multiple_channels_multiple_subscribers_with_unsubscription() {
        let database = get_db_once().await;
        let storage = Storage::new(database.clone());
        let channel_registry = ChannelRegistry::new(storage, "tenant1");

        // Step 1: Create multiple channels
        let channel_1 = "channel_1".to_string();
        let channel_2 = "channel_2".to_string();

        // Step 2: Add subscribers to each channel
        let subscriber_1 = Ulid::new();
        let subscriber_2 = Ulid::new();
        let subscriber_3 = Ulid::new();
        let subscriber_4 = Ulid::new();

        channel_registry
            .subscribe_channel(channel_1.clone(), subscriber_1)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_1.clone(), subscriber_2)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_2.clone(), subscriber_3)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_2.clone(), subscriber_4)
            .await
            .expect("Failed to subscribe");

        // Ensure channels are in the database
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_1)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_2)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );

        // Step 3: Push messages to both channels
        let channel_1_msg_1 = b"Channel 1 Message 1".to_vec();
        let channel_1_msg_2 = b"Channel 1 Message 2".to_vec();
        let channel_2_msg_1 = b"Channel 2 Message 1".to_vec();
        let channel_2_msg_2 = b"Channel 2 Message 2".to_vec();

        channel_registry
            .push_message(channel_1.clone(), channel_1_msg_1.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_1.clone(), channel_1_msg_2.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_2.clone(), channel_2_msg_1.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_2.clone(), channel_2_msg_2.clone())
            .await
            .expect("Failed to push message");

        // Step 4: Ensure all messages can be consumed by respective subscribers
        for subscriber_id in [&subscriber_1, &subscriber_2] {
            assert_eq!(
                channel_registry
                    .consume_message(channel_1.clone(), *subscriber_id)
                    .await
                    .expect("Failed to consume message"),
                Some(channel_1_msg_1.clone())
            );
        }

        for subscriber_id in [&subscriber_1, &subscriber_2] {
            assert_eq!(
                channel_registry
                    .consume_message(channel_1.clone(), *subscriber_id)
                    .await
                    .expect("Failed to consume message"),
                Some(channel_1_msg_2.clone())
            );
        }

        for subscriber_id in [&subscriber_3, &subscriber_4] {
            assert_eq!(
                channel_registry
                    .consume_message(channel_2.clone(), *subscriber_id)
                    .await
                    .expect("Failed to consume message"),
                Some(channel_2_msg_1.clone())
            );
        }

        for subscriber_id in [&subscriber_3, &subscriber_4] {
            assert_eq!(
                channel_registry
                    .consume_message(channel_2.clone(), *subscriber_id)
                    .await
                    .expect("Failed to consume message"),
                Some(channel_2_msg_2.clone())
            );
        }

        // Ensure all queues are empty
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_1)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_2)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );

        // Step 5: Unsubscribe some subscribers and push new messages
        channel_registry
            .unsubscribe_channel(channel_1.clone(), subscriber_2)
            .await
            .expect("Failed to unsubscribe");
        channel_registry
            .unsubscribe_channel(channel_2.clone(), subscriber_3)
            .await
            .expect("Failed to unsubscribe");

        let channel_1_msg_3 = b"Channel 1 Message 3".to_vec();
        let channel_2_msg_3 = b"Channel 2 Message 3".to_vec();

        channel_registry
            .push_message(channel_1.clone(), channel_1_msg_3.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_2.clone(), channel_2_msg_3.clone())
            .await
            .expect("Failed to push message");

        assert_eq!(
            channel_registry
                .consume_message(channel_1.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            Some(channel_1_msg_3.clone())
        );

        // Ensure unsubscribed subscriber cannot consume the message
        assert_eq!(
            channel_registry
                .consume_message(channel_1.clone(), subscriber_2)
                .await
                .expect("Failed to consume message"),
            None
        );

        assert_eq!(
            channel_registry
                .consume_message(channel_2.clone(), subscriber_4)
                .await
                .expect("Failed to consume message"),
            Some(channel_2_msg_3.clone())
        );

        // Ensure unsubscribed subscriber cannot consume the message
        assert_eq!(
            channel_registry
                .consume_message(channel_2.clone(), subscriber_3)
                .await
                .expect("Failed to consume message"),
            None
        );

        // Step 6: Publish messages to non-subscribed channels
        let non_subscribed_channel = "non_subscribed_channel".to_string();
        let unattached_message = b"Message in non-subscribed channel".to_vec();
        channel_registry
            .push_message(non_subscribed_channel.clone(), unattached_message.clone())
            .await
            .expect("Failed to push message");

        // Ensure non-subscribed channel is not created
        assert!(
            channel_registry
                .get_channel_no_lock(&non_subscribed_channel)
                .await
                .expect("Failed to check channel existence")
                .is_none()
        );
    }

    /// This test ensures that channels are properly removed from the database after the
    /// last subscriber has unsubscribed. It validates the following behaviors:
    ///
    /// - When a subscriber unsubscribes from a channel, the channel remains in the database
    ///   if there are still other subscribers.
    /// - If all subscribers unsubscribe from a channel, the channel should be removed from
    ///   the database to free up resources and maintain a clean state.
    /// - The test also verifies that unsubscribing doesn't affect other channels or subscribers.
    #[tokio::test]
    async fn test_subscribers_with_multiple_channels() {
        let database = get_db_once().await;
        let storage = Storage::new(database.clone());
        let channel_registry = ChannelRegistry::new(storage, "tenant1");

        // Step 1: Create multiple channels
        let channel_a = "channel_a".to_string();
        let channel_b = "channel_b".to_string();
        let channel_c = "channel_c".to_string();

        // Step 2: Create subscribers
        let subscriber_1 = Ulid::new();
        let subscriber_2 = Ulid::new();

        // Step 3: Subscribe subscribers to multiple channels
        channel_registry
            .subscribe_channel(channel_a.clone(), subscriber_1)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_b.clone(), subscriber_1)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_c.clone(), subscriber_1)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_b.clone(), subscriber_2)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_c.clone(), subscriber_2)
            .await
            .expect("Failed to subscribe");

        // Ensure channels are in the database with their subscribers
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_a)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_b)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_c)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );

        // Step 4: Push messages to all channels
        let channel_a_msg = b"Message for Channel A".to_vec();
        let channel_b_msg = b"Message for Channel B".to_vec();
        let channel_c_msg = b"Message for Channel C".to_vec();

        channel_registry
            .push_message(channel_a.clone(), channel_a_msg.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_b.clone(), channel_b_msg.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel_c.clone(), channel_c_msg.clone())
            .await
            .expect("Failed to push message");

        // Step 5: Ensure subscriber_1 can consume messages from all its subscribed channels
        assert_eq!(
            channel_registry
                .consume_message(channel_a.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            Some(channel_a_msg.clone())
        );
        assert_eq!(
            channel_registry
                .consume_message(channel_b.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            Some(channel_b_msg.clone())
        );
        assert_eq!(
            channel_registry
                .consume_message(channel_c.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            Some(channel_c_msg.clone())
        );

        // Step 6: Ensure subscriber_2 can consume messages from its subscribed channels
        assert_eq!(
            channel_registry
                .consume_message(channel_b.clone(), subscriber_2)
                .await
                .expect("Failed to consume message"),
            Some(channel_b_msg.clone())
        );
        assert_eq!(
            channel_registry
                .consume_message(channel_c.clone(), subscriber_2)
                .await
                .expect("Failed to consume message"),
            Some(channel_c_msg.clone())
        );

        // Ensure queues are now empty for all channels
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_a)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_b)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_c)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .is_empty()
        );
    }

    /// This test verifies the behavior of a channel_registry that allows multiple subscribers to be
    /// associated with different channels and consume messages independently.
    /// The following scenarios are validated:
    ///
    /// - Subscribers can subscribe to multiple unique channels.
    /// - Channels are correctly initialized in the channel_registry after the first subscription.
    /// - Messages pushed to a channel can be consumed by their respective subscribers.
    /// - Subscribers only consume messages from channels they are subscribed to.
    /// - Channels maintain proper state, ensuring only subscribers of a channel can consume its messages.
    /// - Once all messages are consumed, the queues for all channels are empty.
    #[tokio::test]
    async fn test_channel_removal_after_last_unsubscription() {
        let database = get_db_once().await;
        let storage = Storage::new(database.clone());
        let channel_registry = ChannelRegistry::new(storage, "tenant1");

        // Step 1: Create a channel and add subscribers
        let channel_name = "test_channel".to_string();
        let subscriber_1 = Ulid::new();
        let subscriber_2 = Ulid::new();

        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_1)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel_name.clone(), subscriber_2)
            .await
            .expect("Failed to subscribe");

        // Ensure the channel is in the channel_registry
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );

        // Step 2: Unsubscribe all subscribers
        channel_registry
            .unsubscribe_channel(channel_name.clone(), subscriber_1)
            .await
            .expect("Failed to unsubscribe");
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .is_some()
        ); // Channel should still exist

        channel_registry
            .unsubscribe_channel(channel_name.clone(), subscriber_2)
            .await
            .expect("Failed to unsubscribe");

        // Step 3: Ensure the channel is removed from the channel_registry
        assert!(
            channel_registry
                .get_channel_no_lock(&channel_name)
                .await
                .expect("Unable to get the channel")
                .is_none()
        );
    }

    /// This test verifies the behavior of a system with two subscribers and one publisher
    /// sharing access to a common channel. It ensures that the system maintains a correct
    /// and non-blocking state when one of the subscribers unsubscribes. The following
    /// scenarios are validated:
    ///
    /// - A channel is created and both subscribers are successfully registered.
    /// - Messages are correctly published to the channel.
    /// - Subscribers consume messages and are stopped in their consumption when other subscribers are slower
    /// to consume the current message .
    /// - The system accurately reflects empty queues when all messages are consumed.
    /// - Unsubscribing one subscriber does not block the other from continuing to consume messages.
    #[tokio::test]
    async fn test_two_subscribers_one_publisher_ensure_non_blocking_state_when_subscriber_unsubscribe()
     {
        tracing_subscriber::fmt::init();
        let database = get_db_once().await;
        let storage = Storage::new(database.clone());
        let channel_registry = ChannelRegistry::new(storage, "tenant1");

        // Step 1: Create a channel
        let channel = "shared_channel".to_string();

        // Step 2: Create subscribers
        let subscriber_1 = Ulid::new();
        let subscriber_2 = Ulid::new();

        // Step 3: Subscribe both subscribers to the same channel
        channel_registry
            .subscribe_channel(channel.clone(), subscriber_1)
            .await
            .expect("Failed to subscribe");
        channel_registry
            .subscribe_channel(channel.clone(), subscriber_2)
            .await
            .expect("Failed to subscribe");

        // Ensure the channel is created and the subscribers are registered
        assert!(
            channel_registry
                .get_channel_no_lock(&channel)
                .await
                .expect("Unable to get the channel")
                .is_some()
        );
        assert_eq!(
            channel_registry
                .get_channel_no_lock(&channel)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .subscribers
                .len(),
            2
        );

        // Step 4: Publisher publishes two messages
        let msg_1 = b"First Message".to_vec();
        let msg_2 = b"Second Message".to_vec();

        channel_registry
            .push_message(channel.clone(), msg_1.clone())
            .await
            .expect("Failed to push message");
        channel_registry
            .push_message(channel.clone(), msg_2.clone())
            .await
            .expect("Failed to push message");

        // Step 5: Subscriber 1 consumes the first message
        assert_eq!(
            channel_registry
                .consume_message(channel.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            Some(msg_1.clone())
        );

        warn!("Subscriber 1 consumed message");

        // Step 6: Subscriber 1 attempts to consume again and fails to get an available message
        assert_eq!(
            channel_registry
                .consume_message(channel.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            None
        );

        warn!("Subscriber 1 is done consuming messages");

        // Step 7: Subscriber 2 consumes the first message
        assert_eq!(
            channel_registry
                .consume_message(channel.clone(), subscriber_2)
                .await
                .expect("Failed to consume message"),
            Some(msg_1.clone())
        );

        warn!("Subscriber 2 consumed the first message");

        // Ensure the queue still holds the second message for both subscribers
        assert!(
            channel_registry
                .get_channel_no_lock(&channel)
                .await
                .expect("Unable to get the channel")
                .expect("Channel does not exist")
                .queue
                .contains(&msg_2)
        );

        // Step 8: Subscriber 1 consumes the second message
        assert_eq!(
            channel_registry
                .consume_message(channel.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            Some(msg_2.clone())
        );

        warn!("Subscriber 1 consumed the second message");

        // Step 9: Publisher publishes a third message
        let msg_3 = b"Third Message".to_vec();
        channel_registry
            .push_message(channel.clone(), msg_3.clone())
            .await
            .expect("Failed to push message");

        warn!("Subscriber 1 is still consuming the second message");

        // Step 10: Subscriber 1 consumes a message, but fails to get another one
        assert_eq!(
            channel_registry
                .consume_message(channel.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            None
        );

        warn!("Subscriber 1 is not consuming messages");

        // Step 11: Subscriber 2 unsubscribes from the channel
        channel_registry
            .unsubscribe_channel(channel.clone(), subscriber_2)
            .await
            .expect("Failed to unsubscribe");

        warn!("Subscriber 2 is unsubscribed");

        // Step 12: Subscriber 1 consumes another message, it gets the third message
        assert_eq!(
            channel_registry
                .consume_message(channel.clone(), subscriber_1)
                .await
                .expect("Failed to consume message"),
            Some(msg_3.clone())
        );
    }
}
