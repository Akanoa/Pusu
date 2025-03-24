use crate::parser::errors::ParseResult;
use crate::parser::forecaster::forecast;
use crate::parser::recognizer::recognize;
use crate::parser::scanner::Tokenizer;
use crate::parser::token::Token;
use crate::parser::until_end::UntilEnd;
use crate::parser::visitor::Visitable;
use crate::parser::whitespaces::{OptionalWhitespaces, Whitespaces};

#[derive(Debug, PartialEq)]
pub struct Unsubscribe<'a> {
    pub channel: &'a str,
}

impl<'a> Visitable<'a, u8> for Unsubscribe<'a> {
    fn accept(scanner: &mut Tokenizer<'a>) -> ParseResult<Self> {
        scanner.visit::<OptionalWhitespaces>()?;
        recognize(Token::Unsubscribe, scanner)?;
        // get rid of whitespaces
        scanner.visit::<Whitespaces>()?;
        let token = forecast(UntilEnd, scanner)?;
        let data = token.data;
        let channel = std::str::from_utf8(data)?;
        scanner.bump_by(data.len());
        Ok(Self { channel })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::scanner::Scanner;
    #[test]
    fn test_parse_subscribe() {
        let channel = "my::channel";
        let data = format!("  UNSUBSCRIBE   {channel}");
        let mut scanner = Scanner::new(data.as_bytes());
        let result =
            Unsubscribe::accept(&mut scanner).expect("Unable to parse unsubscribe command");
        assert_eq!(result.channel, channel);

        let data = format!("unsubscribe   {channel}");
        let mut scanner = Scanner::new(data.as_bytes());
        let result =
            Unsubscribe::accept(&mut scanner).expect("Unable to parse unsubscribe command");
        assert_eq!(result.channel, channel);
    }
}
