use crate::parser;
use crate::parser::errors::ParseError;
use crate::parser::scanner::{Scanner, Tokenizer};
use std::fmt::Debug;

/// Attempts to forecast a parsing result from the given tokenizer using the provided element.
///
/// # Type Parameters
/// - `'a`: The lifetime of the tokenizer data being processed.
/// - `U`: The type of the start and end indices, which must implement `Debug`.
/// - `F`: A type implementing the `Forecast` trait, which performs the actual forecasting logic.
///
/// # Parameters
/// - `element`: An instance implementing the `Forecast` trait, used to define how to forecast a result.
/// - `tokenizer`: A mutable reference to the `Tokenizer` that provides the input data and maintains state.
///
/// # Returns
/// - `Ok(Forecasting<'a, u8, U>)`: A successful forecasting result containing the range (start, end),
///    and a slice of the data extracted based on the forecasting.
/// - `Err(ParseError)`: An error when the forecast fails, such as encountering an unexpected token.
///
/// # Errors
/// - `ParseError::UnexpectedToken`: Returned when no valid forecast could be made based on the given input.
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

/// Represents the result of a successful forecast, containing the start and end indices
/// and a slice of the data extracted during the forecasting process.
///
/// # Type Parameters
/// - `'a`: The lifetime of the data being referenced.
/// - `T`: The type of elements in the data slice.
/// - `S`: The type representing the start and end indices.
///
/// # Fields
/// - `start`: The start index of the forecasted range.
/// - `end`: The end index of the forecasted range.
/// - `data`: A reference to a slice of the data within the forecasted range.
///
/// This struct is used to encapsulate information about a forecasted result, including
/// the specific data slice and its corresponding start and end indices, for further processing.
#[derive(Debug, PartialEq)]
pub struct Forecasting<'a, T, S> {
    pub start: S,
    pub end: S,
    pub data: &'a [T],
}

/// A trait for implementing forecasting functionality.
///
/// # Type Parameters
/// - `'a`: The lifetime of the data being processed.
/// - `T`: The type of elements being scanned in the forecast process.
/// - `S`: The type representing the start and end indices of a forecast.
///
/// # Required Methods
/// - [`forecast`](#method.forecast): Attempts to perform a forecast operation on the provided scanner,
///   producing either a successful `ForecastResult` containing the range and slice details, or an error.
///
/// # Errors
/// This trait's `forecast` method can return an error if the forecast process fails for any reason.
/// Errors should be returned within the `ParseResult` type, typically indicating issues
/// such as unexpected input or other parsing-specific problems.
///
/// # Usage
/// Types implementing this trait define how the forecasting logic should be applied
/// based on the input data and the scanning state maintained by the `Scanner`.
pub trait Forecast<'a, T, S> {
    fn forecast(&self, data: &mut Scanner<'a, T>)
    -> parser::errors::ParseResult<ForecastResult<S>>;
}

#[derive(Debug, PartialEq)]
pub enum ForecastResult<S> {
    Found { end_slice: usize, start: S, end: S },
    NotFound,
}
