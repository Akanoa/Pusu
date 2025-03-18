use foundationdb::FdbBindingError;

pub type Result<T> = std::result::Result<T, PusuServerError>;

#[derive(Debug, thiserror::Error)]
pub enum PusuServerError {
    #[error("Storage error: {0}")]
    FoundationDbError(#[from] FdbBindingError),
    #[error("Unable to serialize/deserialize data")]
    BincodeError(#[from] bincode2::Error),
    #[error("Unable to decode UTF-8 string")]
    Utf8Error(#[from] std::str::Utf8Error),
}
