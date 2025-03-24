use crate::parser::errors::ParseResult;
use crate::parser::recognizer::Recognizable;
use crate::parser::scanner::Tokenizer;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Token {
    Auth,
    Subscribe,
    Unsubscribe,
    Publish,
    Consume,
    Quit,
    Whitespace,
    /// Technical Token which never match
    EndOfSlice,
}

pub fn match_char(pattern: char, data: &[u8]) -> (bool, usize) {
    (data[0] == pattern as u8, 1)
}

pub fn match_pattern(pattern: &[u8], data: &[u8]) -> (bool, usize) {
    if pattern.is_empty() {
        return (false, 0);
    }
    if pattern.len() > data.len() {
        return (false, 0);
    }
    if pattern.eq_ignore_ascii_case(&data[..pattern.len()]) {
        return (true, pattern.len());
    }
    (false, 0)
}

pub fn match_predicate<F>(predicate: F, data: &[u8]) -> (bool, usize)
where
    F: Fn(&[u8]) -> (bool, usize),
{
    predicate(data)
}

pub trait Match {
    fn matcher(&self, data: &[u8]) -> (bool, usize);
}

pub trait Size {
    fn size(&self) -> usize;
}

impl Match for Token {
    fn matcher(&self, data: &[u8]) -> (bool, usize) {
        match self {
            Token::Auth => match_pattern(b"AUTH", data),
            Token::Subscribe => match_pattern(b"SUBSCRIBE", data),
            Token::Unsubscribe => match_pattern(b"UNSUBSCRIBE", data),
            Token::Publish => match_pattern(b"PUBLISH", data),
            Token::Consume => match_pattern(b"CONSUME", data),
            Token::Quit => match_pattern(b"QUIT", data),
            Token::Whitespace => match_char(' ', data),
            Token::EndOfSlice => match_predicate(|_| (false, 0), data),
        }
    }
}

impl Size for Token {
    fn size(&self) -> usize {
        match self {
            Token::Auth => 4,
            Token::Subscribe => 9,
            Token::Unsubscribe => 12,
            Token::Publish => 8,
            Token::Consume => 7,
            Token::Quit => 4,
            Token::Whitespace => 1,
            Token::EndOfSlice => 0,
        }
    }
}

impl<'a> Recognizable<'a, &'a [u8]> for Token {
    fn recognize(self, scanner: &mut Tokenizer<'a>) -> ParseResult<Option<&'a [u8]>> {
        let (result, size) = scanner.recognize(self)?;
        if !result {
            return Ok(None);
        }
        let pos = scanner.cursor();
        if !scanner.remaining().is_empty() {
            scanner.bump_by(size);
        }

        Ok(Some(&scanner.data()[pos..pos + size]))
    }
}
