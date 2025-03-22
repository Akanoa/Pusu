pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("An unexpected token has been found")]
    UnexpectedToken,
    #[error("An unexpected end of slice")]
    UnexpectedEOF,
    #[error("Unable to decode string from UTF-8 : {0}")]
    StringUtf8(#[from] std::str::Utf8Error),
}
