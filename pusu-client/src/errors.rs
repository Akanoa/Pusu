use pusu_client_lib::errors::PusuClientLibError;

pub type Result<T> = std::result::Result<T, PusuClientError>;

#[derive(Debug, thiserror::Error)]
pub enum PusuClientError {
    #[error(transparent)]
    PusuClientLibError(#[from] PusuClientLibError),
    #[error(
        "Command Unknown '{0}'accepted commands are: 'quit', 'subscribe', 'unsubscribe', 'publish', 'consume' or 'auth'"
    )]
    UnknownCommand(String),
    #[error(transparent)]
    Parse(#[from] crate::parser::errors::ParseError),
}
