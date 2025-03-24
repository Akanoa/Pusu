use crate::parser::errors::{ParseError, ParseResult};
use crate::parser::scanner::Tokenizer;
use crate::parser::token::Size;

/// `recognize` is a function used to identify and process a recognizable element
/// from the token stream represented by the `scanner`. It ensures that the element
/// can be fully recognized without exceeding the available tokens in the scanner.
///
/// # Parameters
/// - `element`: An instance of a type that implements the `Recognizable` trait.
/// - `scanner`: A mutable reference to the `Tokenizer` instance, representing the
///   token stream from which the element is to be recognized.
///
/// # Returns
/// - `Ok(U)`: If the element is successfully recognized and validated against the token stream.
/// - `Err(ParseError)`: If an error occurs during recognition, such as an unexpected EOF
///   or an unexpected token.
///
/// # Errors
/// - `ParseError::UnexpectedEOF`: Returned if the size of the recognizable element
///   exceeds the remaining length of tokens in the scanner.
/// - `ParseError::UnexpectedToken`: Returned if the element is not recognized as valid
///   by the implementation of the `Recognizable` trait.
pub fn recognize<'a, U, R: Recognizable<'a, U>>(
    element: R,
    scanner: &mut Tokenizer<'a>,
) -> ParseResult<U> {
    if element.size() > scanner.remaining().len() {
        return Err(ParseError::UnexpectedEOF);
    }
    element
        .recognize(scanner)?
        .ok_or(ParseError::UnexpectedToken)
}

/// `Recognizable` is a trait that defines a contract for objects that can be
/// recognized and processed from a token stream represented by the `Tokenizer`.
///
/// Objects implementing this trait must provide the logic necessary to validate
/// themselves against a series of tokens in a token stream.
///
/// # Type Parameters
/// - `'a`: The lifetime associated with the token stream.
/// - `U`: The type of the recognized object to be returned upon successful recognition.
///
/// # Required Methods
/// - `recognize`: This method is responsible for attempting to recognize the implementor
///   from the token stream, updating the stream state if recognition is successful,
///   and returning the recognized object.
/// - `size`: From the `Size` trait, indicating the size of the recognizable element.
///
/// # Errors
/// - The `recognize` method may return `ParseResult::Err` if the token stream does not
///   satisfy the recognition criteria, such as unexpected tokens or insufficient tokens.
pub trait Recognizable<'a, U>: Size {
    fn recognize(self, scanner: &mut Tokenizer<'a>) -> ParseResult<Option<U>>;
}
