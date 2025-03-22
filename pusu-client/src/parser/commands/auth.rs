use crate::parser::errors::ParseResult;
use crate::parser::forecaster::forecast;
use crate::parser::recognizer::Recognizable;
use crate::parser::scanner::Tokenizer;
use crate::parser::token::Token;
use crate::parser::until_end::UntilEnd;
use crate::parser::visitor::Visitable;
use crate::parser::whitespaces::{OptionalWhitespaces, Whitespaces};

#[derive(Debug, PartialEq)]
pub struct Auth<'a> {
    pub token: &'a str,
}

impl<'a> Visitable<'a, u8> for Auth<'a> {
    fn accept(scanner: &mut Tokenizer<'a>) -> ParseResult<Self> {
        scanner.visit::<OptionalWhitespaces>()?;
        Token::Auth.recognize(scanner)?;
        // get rid of whitespaces
        scanner.visit::<Whitespaces>()?;
        let token = forecast(UntilEnd, scanner)?;
        let data = token.data;
        let token = std::str::from_utf8(data)?;
        scanner.bump_by(data.len());
        Ok(Self { token })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::scanner::Scanner;
    #[test]
    fn test_parse_auth() {
        let token = "En0KEwoEMTIzNBgDIgkKBwgIEgMYgAgSJAgAEiC4Q1r9InI-62u8HcND0w6zxWDPTpLmXuKx50pYj5KG1hpAacwj0vAIa3uOW44jyWaMJHH7dd8aK_QPs8LofoRk2ovYHrVNqMp_xCLNpKnt2HFTGCJ3m_3ajUZbWGod2QDoDCIiCiDil5pPWE4vH5OnVI1LgB6KKV9PA1a0F9lbpSFP0U-FaA==";
        let data = format!("  AUTH   {token}");
        let mut scanner = Scanner::new(data.as_bytes());
        let result = Auth::accept(&mut scanner).expect("Unable to parse auth");
        assert_eq!(result.token, token);

        let data = format!("auth   {token}");
        let mut scanner = Scanner::new(data.as_bytes());
        let result = Auth::accept(&mut scanner).expect("Unable to parse auth");
        assert_eq!(result.token, token);
    }
}
