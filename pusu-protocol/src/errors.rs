use thiserror::Error;

pub type Result<T> = std::result::Result<T, PusuProtocolError>;
#[derive(Debug, Error)]
pub enum PusuProtocolError {
    #[error("Unable to encode message")]
    EncodeError(#[from] prost::EncodeError),
    #[error("Unable to decode response")]
    DecodeError(#[from] prost::DecodeError),
}
