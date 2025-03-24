use crate::parser;
use crate::parser::errors::ParseError;
use crate::parser::scanner::{Scanner, Tokenizer};
use std::fmt::Debug;

pub fn forecast<'a, U: Debug, F: Forecast<'a, u8, U>>(
    element: F,
    tokenizer: &mut Tokenizer<'a>,
) -> parser::errors::ParseResult<Forecasting<'a, u8, U>> {
    let source_cursor = tokenizer.cursor();
    if let ForecastResult::Found {
        start,
        end,
        end_slice,
    } = element.forecast(tokenizer)?
    {
        let data = &tokenizer.data()[source_cursor..source_cursor + end_slice];
        return Ok(Forecasting { start, end, data });
    }
    Err(ParseError::UnexpectedToken)
}

#[derive(Debug, PartialEq)]
pub struct Forecasting<'a, T, S> {
    pub start: S,
    pub end: S,
    pub data: &'a [T],
}

pub trait Forecast<'a, T, S> {
    fn forecast(&self, data: &mut Scanner<'a, T>)
    -> parser::errors::ParseResult<ForecastResult<S>>;
}

#[derive(Debug, PartialEq)]
pub enum ForecastResult<S> {
    Found { end_slice: usize, start: S, end: S },
    NotFound,
}
