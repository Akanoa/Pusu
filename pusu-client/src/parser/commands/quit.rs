use crate::parser::errors::ParseResult;
use crate::parser::recognizer::recognize;
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;
use crate::parser::visitor::Visitable;
use crate::parser::whitespaces::OptionalWhitespaces;

#[derive(Debug, PartialEq)]
pub struct Quit;

impl Visitable<'_, u8> for Quit {
    fn accept(scanner: &mut Scanner<'_, u8>) -> ParseResult<Self> {
        scanner.visit::<OptionalWhitespaces>()?;
        recognize(Token::Quit, scanner)?;
        scanner.visit::<OptionalWhitespaces>()?;
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::commands::quit::Quit;
    use crate::parser::scanner::Scanner;
    use crate::parser::visitor::Visitable;

    #[test]
    fn test_quit_command() {
        let data = b" QUIT";
        let mut scanner = Scanner::new(data);
        let result = Quit::accept(&mut scanner).expect("Unable to parse quit command");
        assert_eq!(result, Quit);

        let data = b"  quit";
        let mut scanner = Scanner::new(data);
        let result = Quit::accept(&mut scanner).expect("Unable to parse quit command");
        assert_eq!(result, Quit);
    }
}
