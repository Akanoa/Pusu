use crate::perform_call;
use pusu_protocol::pusu::{MessageResponse, Response};
use pusu_protocol::request::create_consume_request;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use tracing::warn;

pub struct Job {
    channel: String,
    connection: Arc<RwLock<TcpStream>>,
}

impl Job {
    pub fn new(channel: &str, connection: Arc<RwLock<TcpStream>>) -> Self {
        Self {
            channel: channel.to_string(),
            connection,
        }
    }
}

impl Job {
    pub async fn run(
        self,
        sender: UnboundedSender<Vec<u8>>,
        dropper: tokio::sync::oneshot::Receiver<()>,
    ) -> crate::errors::Result<()> {
        let connection = self.connection.clone();
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                if let Ok(Response {
                    response:
                        Some(pusu_protocol::pusu::response::Response::Message(MessageResponse {
                            message: Some(message),
                        })),
                }) = perform_call(
                    create_consume_request(&self.channel).unwrap().as_slice(),
                    connection.clone(),
                )
                .await
                {
                    if let Err(err) = sender.send(message) {
                        warn!(
                            "Unable to send message to the channel {}: {}",
                            self.channel, err
                        )
                    }
                }
            }
        });

        dropper.await?;
        handle.abort();
        Ok(())
    }
}
