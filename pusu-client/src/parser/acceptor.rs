use crate::parser;
use crate::parser::errors::ParseError;
use crate::parser::scanner::Scanner;
use crate::parser::visitor::Visitable;


/// The `Acceptor` struct is a utility that helps manage the process of 
/// recognizing tokens with a scanner and applying transformations to the 
/// recognized data. It allows for constructing a parser mechanism 
/// where various recognizers can be applied, and their results 
/// can be processed incrementally.
///
/// # Type Parameters
/// * `'a` - Lifetime parameter for the data that the `Scanner` operates on.
/// * `'b` - Lifetime parameter for the `Scanner` reference.
/// * `T` - Represents the type of tokens the `Scanner` processes.
/// * `V` - The type of the resulting data after applying transformations.
///
/// # Fields
/// * `data` - An optional field to store the transformed result. 
///   It is initialized as `None` and updated upon successful recognition and transformation.
/// * `scanner` - A mutable reference to the `Scanner` instance responsible for processing input data.
pub struct Acceptor<'a, 'b, T, V> {
    data: Option<V>,
    scanner: &'b mut Scanner<'a, T>,
}

impl<'a, 'b, T, V> Acceptor<'a, 'b, T, V> {
    pub fn new(scanner: &'b mut Scanner<'a, T>) -> Self {
        Self {
            data: None,
            scanner,
        }
    }
}

impl<'a, T, V> Acceptor<'a, '_, T, V> {
    ///
    /// Attempts to apply the given transformer function to the result of a recognized
    /// visitable item from the `Scanner`. If the `data` field already contains a value,
    /// the current state is returned without modification. Otherwise, it tries to recognize
    /// the item using the generic type `U`'s `accept` method.
    ///
    /// If recognition succeeds, the `transformer` function is applied to the recognized item,
    /// and its result is stored in the `data` field. If recognition fails due to an
    /// `UnexpectedToken` error, the scanner's cursor is reset to its original position.
    /// Any other recognition errors are propagated.
    ///
    /// # Type Parameters
    /// * `U` - A type that implements the `Visitable` trait and defines the recognition behavior for the scanner.
    /// * `F` - A callable function or closure used to transform the recognized item into the resulting type `V`.
    ///
    /// # Returns
    /// An `Ok` result wrapping `self` on success, or an error of type `parser::errors::ParseError`
    /// if the transformation or recognition fails.
    pub fn try_or<U: Visitable<'a, T>, F>(
        mut self,
        transformer: F,
    ) -> parser::errors::ParseResult<Self>
    where
        F: Fn(U) -> parser::errors::ParseResult<V>,
    {
        let cursor = self.scanner.cursor();
        // Propagate result
        if self.data.is_some() {
            return Ok(self);
        }

        // Or apply current recognizer
        match U::accept(self.scanner) {
            Ok(found) => {
                self.data = Some(transformer(found)?);
            }
            Err(ParseError::UnexpectedToken) => {
                self.scanner.jump(cursor);
            }
            Err(err) => {
                return Err(err);
            }
        }

        Ok(self)
    }

    
    ///
    /// Consumes the `Acceptor` and returns the final transformed result.
    ///
    /// This method finalizes the parsing and recognition process by extracting the 
    /// transformed data stored in the `Acceptor`. If no transformation was successfully 
    /// applied (i.e., no data is stored in the `data` field), it returns `None`.
    ///
    /// # Returns
    /// An `Option<V>` containing the transformed result if a successful recognition 
    /// and transformation occurred; otherwise, `None`.
    pub fn finish(self) -> Option<V> {
        self.data
    }
}
