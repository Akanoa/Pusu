use crate::errors::PusuClientLibError;
use crate::job::Job;
use futures::{Stream, StreamExt};
use prost::Message;
use pusu_protocol::errors::PusuProtocolError;
use pusu_protocol::pusu::Response;
use pusu_protocol::request::create_auth_request;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::debug;

pub mod errors;
mod job;

pub struct PusuClient {
    connection: Option<Arc<RwLock<TcpStream>>>,
    receiver: Arc<RwLock<tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>>>,
    sender: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    jobs: RwLock<HashMap<String, JobRecord>>,
    broadcaster: tokio::sync::broadcast::Sender<Command>,
}

pub async fn perform_call(
    request: &[u8],
    connection: Arc<RwLock<TcpStream>>,
) -> errors::Result<Response> {
    let mut stream = connection.write().await;
    stream.write_all(request).await?;
    let mut buffer = [0u8; 1024];
    let read_size = stream.read(&mut buffer).await?;
    let response =
        Response::decode(&buffer[..read_size]).map_err(PusuProtocolError::DecodeError)?;
    debug!(response = ?response, "Received response");
    Ok(response)
}

/// Enum representing different commands that can be issued to the PusuClient.
///
/// This enum is used to communicate specific instructions to the client,
/// such as starting or stopping a task. Commands are typically sent through
/// internal broadcast channels to coordinate actions within the client.
///
/// # Variants
///
/// * `Consume` - Signals the client to begin consuming messages or data.
/// * `Stop` - Signals the client to stop all ongoing operations and shutdown.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    Consume,
    Stop,
}

/// A record representing a job managed by the `PusuClient`.
///
/// Each `JobRecord` consists of a `dropper` channel and a `job` handle, which allows
/// controlled shutdown and monitoring of background tasks.
///
/// # Fields
///
/// * `dropper` - An optional `oneshot` channel to signal the termination of the job.
/// * `job` - A handle to the asynchronous task representing the job, which can be awaited or cancelled.
struct JobRecord {
    dropper: Option<tokio::sync::oneshot::Sender<()>>,
    job: tokio::task::JoinHandle<errors::Result<()>>,
}

impl Drop for JobRecord {
    fn drop(&mut self) {
        let _ = self.dropper.take().map(|x| x.send(()));
        self.job.abort();
    }
}

impl Default for PusuClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PusuClient {
    /// Creates and returns a new instance of `PusuClient`.
    ///
    /// This method initializes a new `PusuClient` with default values,
    /// including internal communication channels, job management, and
    /// broadcasting capabilities.
    ///
    /// # Returns
    /// A new instance of `PusuClient` ready to establish connections
    /// and perform operations.
    pub fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        let (broadcast_sender, _) = tokio::sync::broadcast::channel(10);

        let mut blackhole = broadcast_sender.subscribe();

        tokio::spawn(async move { while let Ok(_command) = blackhole.recv().await {} });

        Self {
            connection: None,
            sender,
            receiver: Arc::new(RwLock::new(receiver)),
            jobs: RwLock::new(HashMap::new()),
            broadcaster: broadcast_sender,
        }
    }

    /// Establishes a connection to the server.
    ///
    /// This method attempts to connect to the specified server address and port.
    /// Once the connection is successfully established, it is stored as part of the
    /// client's state for future operations.
    ///
    /// # Parameters
    /// - `addr`: A string slice containing the address of the server.
    /// - `port`: The port number to connect to the server with.
    ///
    /// # Errors
    /// This method will return an error if:
    /// - The client fails to resolve the server address.
    /// - The TCP connection to the specified address and port fails.
    /// - Any I/O operation during the connection setup encounters an error.
    pub async fn connect(&mut self, addr: &str, port: u16) -> errors::Result<()> {
        let addr = format!("{}:{}", addr, port);
        debug!("Connecting to {}", addr);
        let mut stream = TcpStream::connect(&addr).await?;
        debug!("Connected to {}", addr);

        let _ = stream.read(&mut vec![0u8; 1024]).await?;
        debug!("Received handshake");

        self.connection = Some(Arc::new(RwLock::new(stream)));
        Ok(())
    }

    /// Sets a new connection for the client.
    ///
    /// This method replaces the current connection (if any) with the provided `TcpStream`.
    /// It can be used to manually set up or replace the client's connection to a server.
    ///
    /// # Parameters
    /// - `connection`: The new `TcpStream` instance that represents the connection to be used by the client.
    ///
    /// # Errors
    /// This method does not return any errors, but subsequent operations
    /// on the connection may fail if the provided `TcpStream` is invalid
    /// or becomes disconnected.
    pub fn set_connection(&mut self, connection: TcpStream) {
        self.connection = Some(Arc::new(RwLock::new(connection)));
    }

    async fn call(&self, request: &[u8]) -> errors::Result<Response> {
        let connection = self
            .connection
            .as_ref()
            .ok_or(PusuClientLibError::NotConnected)?
            .clone();

        perform_call(request, connection).await
    }

    /// Authenticates the client with the server.
    ///
    /// This method sends an authentication request to the server
    /// using the provided token. Authentication is required
    /// before performing operations that need a verified identity.
    ///
    /// # Parameters
    /// - `token`: A string slice representing the authentication token.
    ///
    /// # Errors
    /// This method will return an error if:
    /// - The client is not connected to the server.
    /// - The authentication request fails to execute.
    /// - The token is invalid or cannot be used to authenticate.
    pub async fn authenticate(&mut self, token: &str) -> errors::Result<()> {
        let auth_request = create_auth_request(token)?;
        self.call(auth_request.as_slice()).await?;
        Ok(())
    }

    /// Subscribes to a specified channel.
    ///
    /// This method will send a subscribe request to the server for the provided channel.
    /// It also sets up a long-running job for receiving messages from the channel,
    /// which will execute in the background.
    ///
    /// # Parameters
    /// - `channel`: A string slice that holds the name of the channel to subscribe to.
    ///
    /// # Errors
    /// This method will return an error if:
    /// - The client is not connected to the server.
    /// - The subscribe request fails to execute.
    /// - The protocol does not create the subscribe request properly.
    pub async fn subscribe(&self, channel: &str) -> errors::Result<()> {
        let subscribe_request = pusu_protocol::request::create_subscribe_request(channel)?;
        self.call(subscribe_request.as_slice()).await?;

        let connection = self
            .connection
            .as_ref()
            .ok_or(PusuClientLibError::NotConnected)?
            .clone();
        let job = Job::new(channel, connection, self.broadcaster.subscribe());

        let (dropper_tx, dropper_rx) = tokio::sync::oneshot::channel();
        let sender = self.sender.clone();

        let handle = tokio::spawn(async move { job.run(sender, dropper_rx).await });

        let job_record = JobRecord {
            job: handle,
            dropper: Some(dropper_tx),
        };

        self.jobs
            .write()
            .await
            .insert(channel.to_string(), job_record);

        Ok(())
    }

    /// Unsubscribes from a specified channel.
    ///
    /// This method sends an unsubscribe request to the server for the specified channel.
    /// It also removes any associated background jobs related to the subscription.
    ///
    /// # Parameters
    /// - `channel`: A string slice containing the name of the channel to unsubscribe from.
    ///
    /// # Errors
    /// This method will return an error if:
    /// - The client is not connected to the server.
    /// - The unsubscribe request fails to execute.
    pub async fn unsubscribe(&mut self, channel: &str) -> errors::Result<()> {
        let unsubscribe_request = pusu_protocol::request::create_unsubscribe_request(channel)?;
        self.call(unsubscribe_request.as_slice()).await?;
        self.jobs.write().await.remove(channel);
        Ok(())
    }

    /// Publishes a message to a specified channel.
    ///
    /// This method sends a publish request to the server for the provided channel
    /// along with the message payload. Other clients subscribed to the channel will
    /// receive the message when the request is processed successfully.
    ///
    /// # Parameters
    /// - `channel`: A string slice representing the name of the channel to which the message is sent.
    /// - `message`: A byte slice representing the payload of the message to be sent.
    ///
    /// # Errors
    /// This method will return an error if:
    /// - The client is not connected to the server.
    /// - The publish request fails to execute.
    /// - The message payload or channel name is invalid.
    pub async fn publish(&mut self, channel: &str, message: &[u8]) -> errors::Result<()> {
        let publish_request = pusu_protocol::request::create_publish_request(channel, message)?;
        self.call(publish_request.as_slice()).await?;
        Ok(())
    }

    /// Gracefully disconnects the client from the server.
    ///
    /// This method sends a quit request to the server to terminate the connection.
    /// Additionally, it stops all background jobs associated with the client.
    ///
    /// # Errors
    /// This method will return an error if:
    /// - The quit request fails to execute.
    /// - The client is not connected to the server.
    pub async fn quit(&mut self) -> errors::Result<()> {
        let quit_request = pusu_protocol::request::create_quit_request()?;
        self.call(quit_request.as_slice()).await?;

        // Kill all jobs in background
        for (_, _job) in self.jobs.write().await.drain() {}

        Ok(())
    }

    pub fn force_consume(&self) {
        self.broadcaster
            .send(Command::Consume)
            .expect("Unable to send command");
    }

    /// Receives messages from the subscribed channels.
    ///
    /// This method initializes a message stream that yields messages
    /// coming from the subscribed channels. It also sends a `Consume`
    /// command to the broadcaster to start consuming messages.
    ///
    /// # Returns
    /// A `MessageIterator` that streams the received messages as a `Vec<u8>`.
    ///
    /// # Errors
    /// This method will propagate error if the `Command::Consume` cannot be sent to the broadcaster.
    pub async fn receive(
        &self,
    ) -> errors::Result<MessageIterator<impl Stream<Item = Vec<u8>> + Unpin>> {
        self.broadcaster.send(Command::Consume)?;
        let broadcaster = self.broadcaster.clone();
        let receiver = self.receiver.clone();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let stream = async_stream::stream! {
            while let Some(message) = receiver.write().await.recv().await {
                yield message;
            }
        };
        let stream = stream.boxed();
        Ok(MessageIterator {
            stream,
            broadcaster,
        })
    }
}

/// A `MessageIterator` is used to stream messages from subscribed channels.
///
/// This struct provides an interface to receive messages in an asynchronous
/// manner, leveraging Rust's async streams. It ensures that the `Command::Stop`
/// is sent to the broadcaster when the iterator is dropped, thereby stopping
/// the consumption of messages.
///
/// # Fields
/// - `stream`: The underlying stream used to receive messages, yielding `Vec<u8>`.
/// - `broadcaster`: A `tokio::sync::broadcast::Sender<Command>` used to control
///   the communication with the broadcaster.
pub struct MessageIterator<S: Stream<Item = Vec<u8>> + Unpin> {
    stream: S,
    broadcaster: tokio::sync::broadcast::Sender<Command>,
}

impl<S: Stream<Item = Vec<u8>> + Unpin> Drop for MessageIterator<S> {
    fn drop(&mut self) {
        self.broadcaster.send(Command::Stop).unwrap();
    }
}

impl<S> Stream for MessageIterator<S>
where
    S: Stream<Item = Vec<u8>> + Unpin,
{
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}

#[cfg(test)]
mod tests {
    use crate::PusuClient;
    use futures::StreamExt;
    use pusu_server_lib::test_utils::Server;
    use std::sync::Arc;
    use tokio::join;
    use tracing::warn;

    /// This test verifies the functionality of the `PusuClient` in a simulated environment
    /// with multiple subscribers and a publisher interacting with a test server.
    /// The goal is to ensure that the message broadcasting, subscription, and reception
    /// mechanisms work as expected.
    ///
    /// The test:
    /// 1. Creates a test server using `pusu_server_lib::test_utils::Server`.
    /// 2. Initializes three `PusuClient` instances: two subscribers and one publisher.
    /// 3. Authenticates each client using a biscuit token obtained from the test server.
    /// 4. Sets up multiple channels to control the test flow. These channels ensure the correct
    ///    sequencing of events between subscribers and the publisher, such as publishing
    ///    messages, subscribing to channels, and verifying message reception.
    /// 5. Runs tasks for the publisher and subscribers using `tokio::spawn` and synchronizes
    ///    them using a barrier and broadcast channels for step-by-step execution.
    /// 6. Publishes messages to different channels and verifies that the correct subscribers
    ///    receive the expected messages.
    /// 7. Simulates scenarios where clients subscribe/unsubscribe to channels, and assesses
    ///    the system's ability to broadcast messages accordingly.
    ///
    /// Key edge cases being tested:
    /// - Proper delivery of messages to subscribed clients.
    /// - Messages are not delivered to unsubscribed clients.
    /// - Correct sequencing of events between clients.
    #[tokio::test]
    async fn test_client() {
        // todo: fix test sometimes infinite loops
        tracing_subscriber::fmt::init();
        let server = Server::new().await;
        let mut subscriber1 = PusuClient::new();
        let mut subscriber2 = PusuClient::new();
        let mut publisher = PusuClient::new();
        let connection1 = server.get_client().await.into_inner();
        let connection2 = server.get_client().await.into_inner();
        let connection3 = server.get_client().await.into_inner();
        let biscuit = server
            .get_biscuit("tenant")
            .to_base64()
            .expect("Unable to encode");

        let channel1 = "channel 1";
        let channel2 = "channel 2";

        let message1 = b"message 1";
        let message2 = b"message 2";
        let message3 = b"message 3";
        let message4 = b"message 4";

        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let (step1_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step2_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step3_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step4_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step5_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step6_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step7_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step8_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step9_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (step10_tx, _) = tokio::sync::broadcast::channel::<()>(1);

        let publisher_task = tokio::spawn({
            let biscuit = biscuit.clone();
            let barrier = barrier.clone();
            let mut step1 = step1_tx.subscribe();
            let step2 = step2_tx.clone();
            let mut step3 = step3_tx.subscribe();
            let step4 = step4_tx.clone();
            let mut step5 = step5_tx.subscribe();
            let step6 = step6_tx.clone();
            let mut step8 = step8_tx.subscribe();
            let step9 = step9_tx.clone();
            async move {
                publisher.set_connection(connection3);
                publisher
                    .authenticate(&biscuit)
                    .await
                    .expect("Authentication failed");

                // wait for subscriber 1 and subscriber 2 to be authenticated
                barrier.wait().await;

                // wait the subscriber 1 to channel
                step1.recv().await.expect("Step 1 failed");

                // publish to channel 1 the message 1
                publisher
                    .publish(channel1, message1)
                    .await
                    .expect("Publishing failed");

                step2.send(()).expect("Unable to send step 2");
                warn!("Sent step 2");

                // wait for subscriber 1 to consume message 1
                step3.recv().await.expect("Step 3 failed");

                // publish to channel 2 the message 2
                publisher
                    .publish(channel2, message2)
                    .await
                    .expect("Publishing failed");
                step4.send(()).expect("Unable to send step 4");
                warn!("Sent step 4");

                // wait for subscriber 1 to subscribe to channel 2
                step5.recv().await.expect("Step 5 failed");
                warn!("Receive step 5");

                // publish to channel 2 the message 3
                publisher
                    .publish(channel2, message3)
                    .await
                    .expect("Publishing failed");
                step6.send(()).expect("Unable to send step 6");
                warn!("Sent step 6");

                // wait for subscriber 1 to consume message 3
                step8.recv().await.expect("Step 8 failed");
                warn!("Receive step 8");

                publisher
                    .publish(channel2, message4)
                    .await
                    .expect("Publishing failed");

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                step9.send(()).expect("Unable to send step 8");
                warn!("Sent step 8");
            }
        });

        let subscriber1_task = tokio::spawn({
            let biscuit = biscuit.clone();
            let barrier = barrier.clone();
            let step1 = step1_tx.clone();
            let mut step2 = step2_tx.subscribe();
            let step3 = step3_tx.clone();
            let mut step4 = step4_tx.subscribe();
            let step5 = step5_tx.clone();
            let mut step6 = step6_tx.subscribe();
            let step7 = step7_tx.clone();
            let mut step10 = step10_tx.subscribe();
            async move {
                subscriber1.set_connection(connection1);
                subscriber1
                    .authenticate(&biscuit)
                    .await
                    .expect("Authentication failed");

                // wait for subscriber 2 and publisher to be authenticated
                barrier.wait().await;

                // subscribe to channel 1 and channel 2
                subscriber1
                    .subscribe(channel1)
                    .await
                    .expect("Subscription failed");

                step1.send(()).unwrap();
                warn!("Sent step 1");

                // wait for publisher to publish message in channel 1
                step2.recv().await.expect("Step 2 failed");

                warn!("Receive step 2");

                // start receiving messages from any subscribed channels
                let mut stream = subscriber1
                    .receive()
                    .await
                    .expect("Unable to receive messages");

                // start receiving message from channel 1
                let message = stream.next().await.expect("Unable to receive message");
                assert_eq!(message, message1);

                step3.send(()).expect("Unable to send step 3");
                warn!("Sent step 3");

                // wait publisher to publish message 2 in channel 2
                step4.recv().await.expect("Step 4 failed");

                // subscribe to channel 2
                subscriber1
                    .subscribe(channel2)
                    .await
                    .expect("Subscription failed");

                step5.send(()).expect("Unable to send step 5");
                warn!("Sent step 5");

                // wait for publisher to publish message 3 to channel 2
                step6.recv().await.expect("Step 6 failed");
                warn!("Receive step 6");

                // start receiving message from channel 2
                subscriber1.force_consume();
                let message = stream.next().await.expect("Unable to receive message");
                assert_eq!(message, message3);

                step7.send(()).expect("Unable to send step 7");
                warn!("Sent step 7");

                // wait for publisher to publish message 4 in channel 2
                step10.recv().await.expect("Step 10 failed");
                warn!("Receive step 10");

                // receive message 4 from channel 2
                let message = stream.next().await.expect("Unable to receive message");
                assert_eq!(message, message4);
            }
        });

        let subscriber2_task = tokio::spawn({
            let biscuit = biscuit.clone();
            let barrier = barrier.clone();
            let mut step7 = step7_tx.subscribe();
            let step8 = step8_tx.clone();
            let mut step9 = step9_tx.subscribe();
            let step10 = step10_tx.clone();
            async move {
                subscriber2.set_connection(connection2);
                subscriber2
                    .authenticate(&biscuit)
                    .await
                    .expect("Authentication failed");

                // wait for subscriber 1 and publisher to be authenticated
                barrier.wait().await;

                // wait for subscriber 1 to consume message 3 from channel 2
                step7.recv().await.expect("Step 7 failed");
                warn!("Receive step 7");

                // subscribe to channel 2
                subscriber2
                    .subscribe(channel2)
                    .await
                    .expect("Subscription failed");

                step8.send(()).expect("Unable to send step 8");
                warn!("Sent step 8");

                // wait for publisher to publish message 4 in channel 2
                step9.recv().await.expect("Step 9 failed");
                warn!("Receive step 9");

                // start receiving messages from any subscribed channels
                let mut stream = subscriber2
                    .receive()
                    .await
                    .expect("Unable to receive messages");

                // receive message 4 from channel 2
                let message = stream.next().await.expect("Unable to receive message");
                assert_eq!(message, message4);

                step10.send(()).expect("Unable to send step 10");
                warn!("Sent step 10");
            }
        });

        let results = join!(subscriber1_task, subscriber2_task, publisher_task);
        results.0.expect("Subscriber 1 failed");
        results.1.expect("Subscriber 2 failed");
        results.2.expect("Publisher failed");
    }

    /// This test verifies multi-tenant behavior for a shared messaging system where messages are
    /// published to a shared channel by different tenants and consumed appropriately based on the
    /// tenant's authentication and subscription. It ensures isolation and correct distribution of
    /// messages among tenants.
    ///
    /// **Test Setup:**
    /// 1. A `Server` is instantiated to simulate the messaging infrastructure.
    /// 2. Two publisher clients and two subscriber clients are created to represent two separate tenants (`tenant1` and `tenant2`).
    /// 3. Each client is assigned its own connection to the server.
    /// 4. Biscuit tokens are generated for each tenant to handle authentication.
    ///
    /// **Test Flow:**
    /// - **Tenant 1:**
    ///   1. Subscribes to the shared channel after being authenticated.
    ///   2. Waits for the publisher under tenant 1 to publish a message.
    ///   3. Receives the message published by tenant 1 and validates it.
    ///
    /// - **Tenant 2:**
    ///   1. Subscribes to the shared channel after being authenticated.
    ///   2. Waits for the publisher under tenant 2 to publish a message.
    ///   3. Receives the message published by tenant 2 and validates it.
    ///
    /// **Parallel Execution:**
    /// The test runs subscriber tasks for both tenants in parallel to simulate multi-tenant operations,
    /// using a synchronization barrier and broadcast channels to coordinate activities.
    ///
    /// **Validations:**
    /// - Message integrity checks ensure that the received messages match the ones published.
    /// - The test relies on `assert_eq!` for comparison and fails if any client misbehaves.
    #[tokio::test]
    async fn test_tenancy() {
        tracing_subscriber::fmt::init();
        let server = Server::new().await;

        // define clients
        let mut subscriber1 = PusuClient::new();
        let mut subscriber2 = PusuClient::new();
        let mut publisher_tenant1 = PusuClient::new();
        let mut publisher_tenant2 = PusuClient::new();

        // define connection
        let connection1 = server.get_client().await.into_inner();
        let connection2 = server.get_client().await.into_inner();
        let connection3 = server.get_client().await.into_inner();
        let connection4 = server.get_client().await.into_inner();

        // define tokens
        let biscuit_tenant1 = server
            .get_biscuit("tenant1")
            .to_base64()
            .expect("Unable to encode");
        let biscuit_tenant2 = server
            .get_biscuit("tenant2")
            .to_base64()
            .expect("Unable to encode");

        // define messages
        let message_tenant_1 = b"message tenant 1";
        let message_tenant_2 = b"message tenant 2";
        let shared_channel = "shared channel";

        let barrier = Arc::new(tokio::sync::Barrier::new(4));
        let (tenant1_tx_step1, _) = tokio::sync::broadcast::channel::<()>(1);
        let (tenant1_tx_step2, _) = tokio::sync::broadcast::channel::<()>(1);
        let (tenant2_tx_step1, _) = tokio::sync::broadcast::channel::<()>(1);
        let (tenant2_tx_step2, _) = tokio::sync::broadcast::channel::<()>(1);

        let subscriber_tenant1_task = tokio::spawn({
            let biscuit = biscuit_tenant1.clone();
            let barrier = barrier.clone();
            let runner = tenant1_tx_step1.clone();
            let mut blocker = tenant1_tx_step2.subscribe();

            async move {
                subscriber1.set_connection(connection1);
                subscriber1
                    .authenticate(&biscuit)
                    .await
                    .expect("Authentication failed");

                // wait for participants to be authenticated
                barrier.wait().await;
                warn!("Tenant 1 subscribing to shared channel");

                subscriber1
                    .subscribe(shared_channel)
                    .await
                    .expect("Subscription failed");

                runner.send(()).expect("Unable to send blocker");
                warn!("Blocker sent tenant 1");

                let mut stream = subscriber1
                    .receive()
                    .await
                    .expect("Unable to receive messages");

                blocker.recv().await.expect("Blocker failed");
                warn!("Blocker received tenant 1");
                subscriber1.force_consume();
                warn!("Force consume tenant 1");
                let message = stream.next().await.expect("Unable to receive message");
                warn!("Message received tenant 1");
                assert_eq!(message, message_tenant_1);
            }
        });

        let subscriber_tenant2_task = tokio::spawn({
            let biscuit = biscuit_tenant2.clone();
            let barrier = barrier.clone();
            let runner = tenant2_tx_step1.clone();
            let mut blocker = tenant2_tx_step2.subscribe();

            async move {
                subscriber2.set_connection(connection2);
                subscriber2
                    .authenticate(&biscuit)
                    .await
                    .expect("Authentication failed");

                // wait for participants to be authenticated
                barrier.wait().await;
                warn!("Tenant 2 subscribing to shared channel");

                subscriber2
                    .subscribe(shared_channel)
                    .await
                    .expect("Subscription failed");

                runner.send(()).expect("Unable to send blocker");
                warn!("Blocker sent tenant 2");

                let mut stream = subscriber2
                    .receive()
                    .await
                    .expect("Unable to receive messages");

                blocker.recv().await.expect("Blocker failed");
                warn!("Blocker received tenant 2");
                subscriber2.force_consume();
                warn!("Force consume tenant 2");
                let message = stream.next().await.expect("Unable to receive message");
                warn!("Message received tenant 2");
                assert_eq!(message, message_tenant_2);
            }
        });

        let publisher_tenant1_task = tokio::spawn({
            let biscuit = biscuit_tenant1.clone();
            let barrier = barrier.clone();
            let mut blocker = tenant1_tx_step1.subscribe();
            let runner = tenant1_tx_step2.clone();

            async move {
                publisher_tenant1.set_connection(connection3);
                publisher_tenant1
                    .authenticate(&biscuit)
                    .await
                    .expect("Authentication failed");

                // wait for participants to be authenticated
                barrier.wait().await;

                // wait for subscriber to subscribe
                blocker.recv().await.expect("Blocker failed");
                warn!("Blocker received tenant 1");

                publisher_tenant1
                    .publish(shared_channel, message_tenant_1)
                    .await
                    .expect("Publishing failed");

                runner.send(()).expect("Unable to send blocker");
            }
        });

        let publisher_tenant2_task = tokio::spawn({
            let biscuit = biscuit_tenant2.clone();
            let barrier = barrier.clone();
            let mut blocker = tenant2_tx_step1.subscribe();
            let runner = tenant2_tx_step2.clone();

            async move {
                publisher_tenant2.set_connection(connection4);
                publisher_tenant2
                    .authenticate(&biscuit)
                    .await
                    .expect("Authentication failed");

                // wait for participants to be authenticated
                barrier.wait().await;
                blocker.recv().await.expect("Blocker failed");
                warn!("Blocker received tenant 2");

                publisher_tenant2
                    .publish(shared_channel, message_tenant_2)
                    .await
                    .expect("Publishing failed");

                runner.send(()).expect("Unable to send blocker");
            }
        });

        let results = join!(
            subscriber_tenant1_task,
            subscriber_tenant2_task,
            publisher_tenant1_task,
            publisher_tenant2_task
        );
        results.0.expect("Subscriber 1 failed");
        results.1.expect("Subscriber 2 failed");
        results.2.expect("Publisher 1 failed");
        results.3.expect("Publisher 2 failed");
    }
}
