use foundationdb::FdbBindingError;

pub type Result<T> = std::result::Result<T, PusuServerError>;

#[derive(Debug, thiserror::Error)]
pub enum PusuServerError {
    #[error("Storage error: {0}")]
    FoundationDbError(#[from] FdbBindingError),
}
