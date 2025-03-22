use crate::{perform_call, Command};
use pusu_protocol::pusu::{MessageResponse, Response};
use pusu_protocol::request::create_consume_request;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use tracing::{debug, warn};

pub struct Job {
    channel: String,
    connection: Arc<RwLock<TcpStream>>,
    receiver: tokio::sync::broadcast::Receiver<Command>,
}

impl Job {
    pub fn new(
        channel: &str,
        connection: Arc<RwLock<TcpStream>>,
        receiver: tokio::sync::broadcast::Receiver<Command>,
    ) -> Self {
        Self {
            channel: channel.to_string(),
            connection,
            receiver,
        }
    }
}

async fn consume(
    connection: Arc<RwLock<TcpStream>>,
    sender: UnboundedSender<Vec<u8>>,
    channel: String,
) -> crate::errors::Result<()> {
    debug!(channel, "Starting consume for channel");
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        if let Ok(Response {
            response:
                Some(pusu_protocol::pusu::response::Response::Message(MessageResponse {
                    message: Some(message),
                })),
        }) = perform_call(
            create_consume_request(&channel)?.as_slice(),
            connection.clone(),
        )
        .await
        {
            if let Err(err) = sender.send(message) {
                warn!("Unable to send message to the channel {}: {}", channel, err)
            }
        }
    }
}

impl Job {
    pub async fn run(
        mut self,
        sender: UnboundedSender<Vec<u8>>,
        mut dropper: tokio::sync::oneshot::Receiver<()>,
    ) -> crate::errors::Result<()> {
        let connection = self.connection.clone();

        let mut handle = None;

        loop {
            select! {
                // Kill the job
                _ = &mut dropper => {
                    break
                }
                // Wait for Pusu Client command
                command = self.receiver.recv() => {
                    match command.unwrap() {
                        Command::Consume => {
                            debug!("Starting worker");
                            // start the worker only if not exist yet
                            if handle.is_none() {
                                // start the background job
                                handle = Some(tokio::spawn(consume(connection.clone(), sender.clone(), self.channel.to_string())));
                            }
                        }
                        Command::Stop => {
                            debug!("Stopping worker");
                            // stop the worker if it's started
                            if let Some(handle) = handle {
                                handle.abort();
                            }
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
