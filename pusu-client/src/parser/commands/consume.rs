use crate::parser::errors::ParseResult;
use crate::parser::recognizer::recognize;
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;
use crate::parser::visitor::Visitable;
use crate::parser::whitespaces::OptionalWhitespaces;

#[derive(Debug, PartialEq)]
pub struct Consume;

impl Visitable<'_, u8> for Consume {
    fn accept(scanner: &mut Scanner<'_, u8>) -> ParseResult<Self> {
        scanner.visit::<OptionalWhitespaces>()?;
        recognize(Token::Consume, scanner)?;
        scanner.visit::<OptionalWhitespaces>()?;
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::commands::consume::Consume;
    use crate::parser::scanner::Scanner;
    use crate::parser::visitor::Visitable;

    #[test]
    fn test_consume() {
        let data = b" CONSUME";
        let mut scanner = Scanner::new(data);
        let result = Consume::accept(&mut scanner).expect("Unable to parse consume command");
        assert_eq!(result, Consume);

        let data = b"  consume";
        let mut scanner = Scanner::new(data);
        let result = Consume::accept(&mut scanner).expect("Unable to parse consume command");
        assert_eq!(result, Consume);
    }
}
