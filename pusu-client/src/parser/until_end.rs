use crate::parser::forecaster::{Forecast, ForecastResult};
use crate::parser::scanner::Scanner;
use crate::parser::token::Token;

/// A structure that provides a forecast implementation which spans until the end of the input slice.
///
/// # Purpose
/// `UntilEnd` is used to determine the "forecast" result for a given `Scanner`
/// until no data remains. Its primary use is for scenarios requiring processing
/// or evaluation of the entire remaining input.
///
/// # Related Components
/// - [`Scanner`]: Parses input data to track progress.
/// - [`Forecast`]: A trait implemented by this structure.
/// - [`ForecastResult`]: Enum representing the result of the forecast.
///
/// # Notes
/// - This struct works on `u8` slices and generates a forecast result using `Token::EndOfSlice`.
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
