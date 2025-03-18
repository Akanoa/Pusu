use crate::errors::PusuServerError;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct Duplex<R, W> {
    pub read: Receiver<R>,
    pub write: Sender<W>,
}

impl<R, W> Duplex<R, W> {
    pub async fn write(&self, message: W) -> crate::errors::Result<()> {
        self.write.send(message).await.map_err(|err| {
            tracing::error!(?err, "Unable to send response");
            PusuServerError::UnableToSendResponse
        })?;
        Ok(())
    }

    pub async fn read(&mut self) -> crate::errors::Result<R> {
        match self.read.recv().await {
            None => {
                tracing::warn!("Channel closed");
                Err(PusuServerError::ChannelClosed)
            }
            Some(data) => Ok(data),
        }
    }
}
