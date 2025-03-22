use crate::parser::errors::ParseResult;
use crate::parser::forecaster::forecast;
use crate::parser::recognizer::Recognizable;
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;
use crate::parser::until_end::UntilEnd;
use crate::parser::until_token::UntilToken;
use crate::parser::visitor::Visitable;
use crate::parser::whitespaces::{OptionalWhitespaces, Whitespaces};

#[derive(Debug, PartialEq)]
pub struct Publish<'a> {
    pub channel: &'a str,
    pub message: &'a [u8],
}

impl<'a> Visitable<'a, u8> for Publish<'a> {
    fn accept(scanner: &mut Scanner<'a, u8>) -> ParseResult<Self> {
        scanner.visit::<OptionalWhitespaces>()?;
        Token::Publish.recognize(scanner)?;
        // get rid of whitespaces
        scanner.visit::<Whitespaces>()?;
        Token::Channel.recognize(scanner)?;
        scanner.visit::<Whitespaces>()?;
        let token = forecast(UntilToken(Token::Whitespace), scanner)?;
        let data = token.data;
        let channel = std::str::from_utf8(data)?;
        scanner.bump_by(data.len());
        scanner.visit::<Whitespaces>()?;
        Token::Message.recognize(scanner)?;
        scanner.visit::<Whitespaces>()?;
        let token = forecast(UntilEnd, scanner)?;
        let message = token.data;
        scanner.bump_by(message.len());
        Ok(Self { channel, message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::scanner::Scanner;
    #[test]
    fn test_parse_publish() {
        let channel = "my::channel";
        let message = "Hello world";
        let data = format!("  PUBLISH CHANNEL {channel} MESSAGE {message}");
        let mut scanner = Scanner::new(data.as_bytes());
        let result = Publish::accept(&mut scanner).expect("Unable to parse publish command");
        assert_eq!(result.channel, channel);
        assert_eq!(result.message, message.as_bytes());

        let data = format!("  publish channel {channel} message {message}");
        let mut scanner = Scanner::new(data.as_bytes());
        let result = Publish::accept(&mut scanner).expect("Unable to parse publish command");
        assert_eq!(result.channel, channel);
        assert_eq!(result.message, message.as_bytes());
    }
}
