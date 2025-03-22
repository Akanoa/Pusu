use crate::parser::errors::{ParseError, ParseResult};
use crate::parser::scanner::Tokenizer;
use crate::parser::token::Size;

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

pub trait Recognizable<'a, U>: Size {
    fn recognize(self, scanner: &mut Tokenizer<'a>) -> ParseResult<Option<U>>;
}
