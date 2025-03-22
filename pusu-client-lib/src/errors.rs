use crate::Command;
use tokio::sync::broadcast::error::SendError;

pub type Result<T> = std::result::Result<T, PusuClientLibError>;

#[derive(Debug, thiserror::Error)]
pub enum PusuClientLibError {
    #[error("Unable to connect to the server")]
    ConnectionError,
    #[error("I∕O error : {0}")]
    IoError(#[from] std::io::Error),
    #[error("Protocol error: {0}")]
    Protocol(#[from] pusu_protocol::errors::PusuProtocolError),
    #[error("Not connected")]
    NotConnected,
    #[error("Job error: {0}")]
    JobError(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("Unable to send command")]
    SendCommandError(#[from] SendError<Command>),
}
