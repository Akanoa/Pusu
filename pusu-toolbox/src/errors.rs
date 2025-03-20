pub type Result<T> = std::result::Result<T, PusuToolboxError>;

#[derive(Debug, thiserror::Error)]
pub enum PusuToolboxError {
    #[error("Biscuit error : {0}")]
    Biscuit(#[from] biscuit_auth::error::Token),
    #[error("Biscuit creation error : {0}")]
    BiscuitCreation(#[from] biscuit_auth::error::Format),
}
