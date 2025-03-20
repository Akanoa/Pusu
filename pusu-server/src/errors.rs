use pusu_server_lib::errors::PusuServerLibError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, PusuServerError>;

#[derive(Debug, Error)]
pub enum PusuServerError {
    #[error("Invalid configuration : {0}")]
    ConfigurationError(#[from] figment::Error),
    #[error(transparent)]
    PusuLibError(#[from] PusuServerLibError),
    #[error("An error occured with FoundationDB : {0}")]
    FdbBindingError(#[from] foundationdb::FdbBindingError),
    #[error("Unable to initialize the database : {0}")]
    FdbError(#[from] foundationdb::FdbError),
    #[error("Invalid public key")]
    InvalidPublicKey(#[from] biscuit_auth::error::Format),
    #[error("I∕O error : {0}")]
    Io(#[from] std::io::Error),
}
