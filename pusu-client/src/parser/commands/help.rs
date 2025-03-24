use crate::parser::errors::ParseResult;
use crate::parser::recognizer::recognize;
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;
use crate::parser::visitor::Visitable;
use crate::parser::whitespaces::OptionalWhitespaces;

#[derive(Debug, PartialEq)]
pub struct Help;

impl Visitable<'_, u8> for Help {
    fn accept(scanner: &mut Scanner<'_, u8>) -> ParseResult<Self> {
        scanner.visit::<OptionalWhitespaces>()?;
        recognize(Token::Help, scanner)?;
        scanner.visit::<OptionalWhitespaces>()?;
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::commands::help::Help;
    use crate::parser::scanner::Scanner;
    use crate::parser::visitor::Visitable;

    #[test]
    fn test_help_command() {
        let data = b" HELP";
        let mut scanner = Scanner::new(data);
        let result = Help::accept(&mut scanner).expect("Unable to parse help command");
        assert_eq!(result, Help);

        let data = b"  help";
        let mut scanner = Scanner::new(data);
        let result = Help::accept(&mut scanner).expect("Unable to parse help command");
        assert_eq!(result, Help);
    }
}
