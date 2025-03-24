use crate::parser::errors::{ParseError, ParseResult};
use crate::parser::token::Match;
use crate::parser::visitor::Visitable;
use std::io::Cursor;

/// `Scanner` is a generic structure that provides functionality to scan
/// through a slice of data iteratively. It maintains a cursor to track
/// the current position within the data slice.
///
/// # Type Parameters
/// - `T`: The type of the elements in the data slice.
///
/// # Lifetime Parameters
/// - `'a`: The lifetime of the data slice reference.
///
/// # Fields
/// - `cursor`: A `Cursor` that tracks the current position within the slice.
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
    /// Advances the cursor by a specified number of elements.
    ///
    /// # Parameters
    /// - `size`: The number of elements to move the cursor forward by.
    pub fn bump_by(&mut self, size: usize) {
        self.cursor
            .set_position(self.cursor.position() + size as u64)
    }

    /// Returns the remaining unprocessed portion of the data slice.
    ///
    /// # Returns
    /// A slice containing the data from the current cursor position to the end.
    pub fn remaining(&self) -> &'a [T] {
        &self.cursor.get_ref()[self.cursor.position() as usize..]
    }

    /// Returns the entire data slice being scanned.
    ///
    /// # Returns
    /// A slice containing the full data passed to the scanner.
    pub fn data(&self) -> &'a [T] {
        self.cursor.get_ref()
    }

    /// Returns the current position of the cursor within the data slice.
    ///
    /// # Returns
    /// The index of the cursor within the data slice.
    pub fn cursor(&self) -> usize {
        self.cursor.position() as usize
    }

    /// Moves the cursor to a specific position within the data slice.
    ///
    /// # Parameters
    /// - `pos`: The index to set the cursor position to.
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
    /// Recognizes a pattern in the remaining data slice using the provided `matcher`.
    ///
    /// The method attempts to match a pattern within the unprocessed portion
    /// of the data slice. It uses the provided `matcher` to perform the matching.
    ///
    /// # Parameters
    /// - `matcher`: A `Match`-implementing type used to match a pattern against the data slice.
    ///
    /// # Returns
    /// - `Ok((bool, usize))`: A tuple containing a boolean indicating if a match was found
    ///   and the size of the match in bytes.
    /// - `Err(ParseError)`: If the end of the data is reached unexpectedly, an error
    ///   of type `ParseError::UnexpectedEOF` is returned.
    ///
    /// # Errors
    /// - Returns a `ParseError::UnexpectedEOF` if the remaining data slice is empty.
    pub fn recognize(&mut self, matcher: impl Match) -> ParseResult<(bool, usize)>
where {
        if self.remaining().is_empty() {
            return Err(ParseError::UnexpectedEOF);
        }

        let data = self.remaining();
        Ok(matcher.matcher(data))
    }
}
