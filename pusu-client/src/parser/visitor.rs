use crate::parser::errors::ParseResult;
use crate::parser::scanner::Scanner;

pub trait Visitable<'a, T>: Sized {
    fn accept(scanner: &mut Scanner<'a, T>) -> ParseResult<Self>;
}
