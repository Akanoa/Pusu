use thiserror::Error;

pub type Result<T> = std::result::Result<T, PusuprotocolError>;
#[derive(Debug, Error)]
pub enum PusuprotocolError {
    #[error("Unable to encode message")]
    UnableToEncodeMessage(#[from] prost::EncodeError),
    #[error("Unable to decode response")]
    UnableToDecodeResponse(#[from] prost::DecodeError),
}
