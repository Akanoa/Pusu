use crate::parser::errors::{ParseError, ParseResult};
use crate::parser::token::Match;
use crate::parser::visitor::Visitable;
use std::io::Cursor;

pub struct Scanner<'a, T> {
    cursor: Cursor<&'a [T]>,
}

impl<'a, T> Scanner<'a, T> {
    pub(crate) fn new(input: &'a [T]) -> Self {
        Self {
            cursor: Cursor::new(input),
        }
    }
}

impl<'a, T> Scanner<'a, T> {
    pub fn bump_by(&mut self, size: usize) {
        self.cursor
            .set_position(self.cursor.position() + size as u64)
    }

    pub fn remaining(&self) -> &'a [T] {
        &self.cursor.get_ref()[self.cursor.position() as usize..]
    }

    pub fn data(&self) -> &'a [T] {
        self.cursor.get_ref()
    }

    pub fn cursor(&self) -> usize {
        self.cursor.position() as usize
    }

    pub fn jump(&mut self, pos: usize) {
        self.cursor.set_position(pos as u64);
    }
}

impl<'a, T> Scanner<'a, T> {
    pub(crate) fn visit<U: Visitable<'a, T>>(&mut self) -> ParseResult<U> {
        U::accept(self)
    }
}

pub type Tokenizer<'a> = Scanner<'a, u8>;

impl Tokenizer<'_> {
    pub fn recognize(&mut self, matcher: impl Match) -> ParseResult<(bool, usize)>
where {
        if self.remaining().is_empty() {
            return Err(ParseError::UnexpectedEOF);
        }

        let data = self.remaining();
        Ok(matcher.matcher(data))
    }
}
