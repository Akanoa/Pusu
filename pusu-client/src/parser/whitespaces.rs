use crate::parser;
use crate::parser::errors::ParseError;
use crate::parser::recognizer::Recognizable;
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;
use crate::parser::visitor::Visitable;

/// Represents a sequence of whitespaces in the input.
///
/// The `Whitespaces` structure is used to parse a mandatory sequence
/// of whitespace characters in the input stream. It implements the
/// `Visitable` trait, allowing it to recognize and accept these characters.
///
/// # Errors
/// - Returns `ParseError::UnexpectedToken` if no whitespaces are found in the input.
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

///
/// Represents optional whitespaces in the input.
///
/// The `OptionalWhitespaces` structure is used to parse a sequence of
/// whitespace characters that are optional in the input stream. This
/// structure is particularly useful in contexts where the presence of
/// whitespaces is allowed but not mandatory.
///
/// It implements the `Visitable` trait, enabling it to detect and
/// handle such characters without throwing an error if none are found.
///
/// # Notes
/// - If no whitespaces are found, the structure still returns successfully.
///
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
