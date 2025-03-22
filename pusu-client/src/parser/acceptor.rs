use crate::parser;
use crate::parser::errors::ParseError;
use crate::parser::scanner::Scanner;
use crate::parser::visitor::Visitable;

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

impl<'a, 'b, T, V> Acceptor<'a, 'b, T, V> {
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

    pub fn finish(self) -> Option<V> {
        self.data
    }
}
