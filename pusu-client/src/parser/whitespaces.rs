use crate::parser;
use crate::parser::errors::ParseError;
use crate::parser::recognizer::Recognizable;
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;
use crate::parser::visitor::Visitable;

#[derive(Debug)]
pub struct Whitespaces;

impl<'a> Visitable<'a, u8> for Whitespaces {
    fn accept(scanner: &mut Scanner<'a, u8>) -> parser::errors::ParseResult<Self> {
        let mut counter = 0;
        while Token::Whitespace.recognize(scanner)?.is_some() {
            counter += 1;
        }
        if counter == 0 {
            return Err(ParseError::UnexpectedToken);
        }
        Ok(Whitespaces)
    }
}

#[derive(Debug)]
pub struct OptionalWhitespaces;

impl<'a> Visitable<'a, u8> for OptionalWhitespaces {
    fn accept(scanner: &mut Scanner<'a, u8>) -> parser::errors::ParseResult<Self> {
        if scanner.remaining().is_empty() {
            return Ok(OptionalWhitespaces);
        }

        while Token::Whitespace.recognize(scanner)?.is_some() {
            if scanner.remaining().is_empty() {
                break;
            }
        }
        Ok(OptionalWhitespaces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::scanner::Tokenizer;

    #[test]
    fn test_whitespaces() {
        let data = b"    data";
        let mut tokenizer = Tokenizer::new(data);
        tokenizer
            .visit::<Whitespaces>()
            .expect("failed to parse whitespaces");
        assert_eq!(tokenizer.remaining(), b"data");
    }

    #[test]
    fn test_whitespaces_fail() {
        let data = b"data";
        let mut tokenizer = Tokenizer::new(data);
        assert!(tokenizer.visit::<Whitespaces>().is_err());
    }
}
