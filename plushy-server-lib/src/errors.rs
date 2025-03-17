use foundationdb::FdbBindingError;

pub type Result<T> = std::result::Result<T, PlushyServerError>;

#[derive(Debug, thiserror::Error)]
pub enum PlushyServerError {
    #[error("Storage error: {0}")]
    FoundationDbError(#[from] FdbBindingError),
}
