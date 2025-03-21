use crate::errors::PusuClientLibError;
use crate::job::Job;
use futures::Stream;
use prost::Message;
use pusu_protocol::errors::PusuProtocolError;
use pusu_protocol::pusu::Response;
use pusu_protocol::request::create_auth_request;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;

mod errors;
mod job;

pub struct PusuClient {
    connection: Option<Arc<RwLock<TcpStream>>>,
    receiver: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    sender: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    jobs: HashMap<String, JobRecord>,
}

pub async fn perform_call(
    request: &[u8],
    connection: Arc<RwLock<TcpStream>>,
) -> errors::Result<Response> {
    let mut stream = connection.write().await;
    stream.write_all(request).await?;
    let mut buffer = [0u8; 1024];
    let _ = stream.read(&mut buffer).await?;
    let response = Response::decode(&buffer[..]).map_err(PusuProtocolError::DecodeError)?;
    Ok(response)
}

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
    pub fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        Self {
            connection: None,
            sender,
            receiver,
            jobs: HashMap::new(),
        }
    }

    pub async fn connect(&mut self, addr: &str, port: u16) -> errors::Result<()> {
        let addr = format!("{}:{}", addr, port);
        let stream = TcpStream::connect(format!("{addr}:{port}")).await?;

        tokio::task::spawn_local(async {});

        self.connection = Some(Arc::new(RwLock::new(stream)));
        Ok(())
    }

    pub fn set_connection(&mut self, connection: TcpStream) {
        self.connection = Some(Arc::new(RwLock::new(connection)));
    }

    async fn call(&mut self, request: &[u8]) -> errors::Result<Response> {
        let connection = self
            .connection
            .as_mut()
            .ok_or(PusuClientLibError::NotConnected)?
            .clone();

        perform_call(request, connection).await
    }

    pub async fn authenticate(&mut self, token: &str) -> errors::Result<()> {
        let auth_request = create_auth_request(token)?;
        self.call(auth_request.as_slice()).await?;
        Ok(())
    }

    pub async fn subscribe(&mut self, channel: &str) -> errors::Result<()> {
        let subscribe_request = pusu_protocol::request::create_subscribe_request(channel)?;
        self.call(subscribe_request.as_slice()).await?;

        let job = Job::new(channel, self.connection.clone().unwrap());
        let (dropper_tx, dropper_rx) = tokio::sync::oneshot::channel();
        let sender = self.sender.clone();

        let handle = tokio::spawn(async move { job.run(sender, dropper_rx).await });

        let job_record = JobRecord {
            job: handle,
            dropper: Some(dropper_tx),
        };

        self.jobs.insert(channel.to_string(), job_record);

        Ok(())
    }

    pub async fn unsubscribe(&mut self, channel: &str) -> errors::Result<()> {
        let unsubscribe_request = pusu_protocol::request::create_unsubscribe_request(channel)?;
        self.call(unsubscribe_request.as_slice()).await?;
        self.jobs.remove(channel);
        Ok(())
    }

    pub async fn publish(&mut self, channel: &str, message: &[u8]) -> errors::Result<()> {
        let publish_request = pusu_protocol::request::create_publish_request(channel, message)?;
        self.call(publish_request.as_slice()).await?;
        Ok(())
    }

    pub async fn quit(&mut self) -> errors::Result<()> {
        let quit_request = pusu_protocol::request::create_quit_request()?;
        self.call(quit_request.as_slice()).await?;
        Ok(())
    }

    pub async fn receive(&mut self) -> impl Stream<Item = Vec<u8>> {
        async_stream::stream! {
            while let Some(message) = self.receiver.recv().await {
                yield message;
            }
        }
    }
}
