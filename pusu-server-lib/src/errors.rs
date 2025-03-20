use foundationdb::FdbBindingError;

pub type Result<T> = std::result::Result<T, PusuServerLibError>;

#[derive(Debug, thiserror::Error)]
pub enum PusuServerLibError {
    #[error("Storage error: {0}")]
    FoundationDbError(#[from] FdbBindingError),
    #[error("Unable to serialize/deserialize data")]
    BincodeError(#[from] bincode2::Error),
    #[error("Unable to decode UTF-8 string")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("Please authenticate first")]
    NotAuthenticated,
    #[error(transparent)]
    ProtocolError(#[from] pusu_protocol::errors::PusuprotocolError),
    #[error("Unable to send response to client")]
    UnableToSendResponse,
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Unable to verify biscuit token : {0}")]
    UnableToVerifyBiscuitToken(#[from] biscuit_auth::error::Token),
    #[error("Malformed biscuit missing tenant fact")]
    MalformedBiscuitMissingTenantFact,
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}
