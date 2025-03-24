use crate::parser::errors::ParseResult;
use crate::parser::scanner::Scanner;

/// A trait that allows a type to be "visited" by a [`Scanner`].
///
/// The `Visitable` trait defines a contract for types that can be
/// parsed or processed using a scanner. Implementing this trait
/// requires defining how the type is accepted and parsed from a
/// mutable reference to a [`Scanner`].
///
/// # Type Parameters
///
/// - `'a`: Lifetime parameter specifying the lifetime of the scanner's input.
/// - `T`: The type of the tokens or input being scanned.
///
/// # Associated Function
///
/// - `accept`: A method that takes a mutable reference to a [`Scanner`]
///   and attempts to parse the implementing type, returning a [`ParseResult`].
///
/// [`Scanner`]: Scanner
/// [`ParseResult`]: ParseResult
pub trait Visitable<'a, T>: Sized {
    fn accept(scanner: &mut Scanner<'a, T>) -> ParseResult<Self>;
}
