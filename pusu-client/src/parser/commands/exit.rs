use crate::parser::errors::ParseResult;
use crate::parser::recognizer::recognize;
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;
use crate::parser::visitor::Visitable;
use crate::parser::whitespaces::OptionalWhitespaces;

#[derive(Debug, PartialEq)]
pub struct Exit;

impl Visitable<'_, u8> for Exit {
    fn accept(scanner: &mut Scanner<'_, u8>) -> ParseResult<Self> {
        scanner.visit::<OptionalWhitespaces>()?;
        recognize(Token::Exit, scanner)?;
        scanner.visit::<OptionalWhitespaces>()?;
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::commands::exit::Exit;
    use crate::parser::scanner::Scanner;
    use crate::parser::visitor::Visitable;

    #[test]
    fn test_help_command() {
        let data = b" EXIT";
        let mut scanner = Scanner::new(data);
        let result = Exit::accept(&mut scanner).expect("Unable to parse exit command");
        assert_eq!(result, Exit);

        let data = b"  exit";
        let mut scanner = Scanner::new(data);
        let result = Exit::accept(&mut scanner).expect("Unable to parse exit command");
        assert_eq!(result, Exit);
    }
}
