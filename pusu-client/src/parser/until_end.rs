use crate::parser::forecaster::{Forecast, ForecastResult};
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;

pub struct UntilEnd;

impl<'a> Forecast<'a, u8, Token> for UntilEnd {
    fn forecast(
        &self,
        data: &mut Scanner<'a, u8>,
    ) -> crate::parser::errors::ParseResult<ForecastResult<Token>> {
        Ok(ForecastResult::Found {
            end_slice: data.remaining().len(),
            start: Token::EndOfSlice,
            end: Token::EndOfSlice,
        })
    }
}
