use crate::parser::forecaster::{Forecast, ForecastResult};
use crate::parser::recognizer::recognize;
use crate::parser::scanner::{Scanner, Tokenizer};
use crate::parser::token::{Size, Token};

/// `UntilToken` is a structure that wraps a `Token` and is used to forecast
/// the presence of that `Token` in a stream of data. It provides logic to
/// scan through the data and determine whether the wrapped `Token` appears,
/// returning the result as a `ForecastResult`.
pub struct UntilToken(pub Token);

impl<'a> Forecast<'a, u8, Token> for UntilToken {
    fn forecast(
        &self,
        data: &mut Scanner<'a, u8>,
    ) -> crate::parser::errors::ParseResult<ForecastResult<Token>> {
        let mut tokenizer = Tokenizer::new(data.remaining());

        while !tokenizer.remaining().is_empty() {
            match recognize(self.0, &mut tokenizer) {
                Ok(_element) => {
                    return Ok(ForecastResult::Found {
                        end_slice: tokenizer.cursor() - self.0.size(),
                        start: self.0,
                        end: self.0,
                    });
                }
                Err(_err) => {
                    tokenizer.bump_by(1);
                    continue;
                }
            }
        }
        Ok(ForecastResult::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::forecaster::{Forecasting, forecast};
    use crate::parser::scanner::Tokenizer;
    use crate::parser::token::Token;
    use crate::parser::until_token::UntilToken;

    #[test]
    fn test_until_token() {
        let data = b"id, brand FROM cars";
        let mut tokenizer = Tokenizer::new(data);
        let forecast_result =
            forecast(UntilToken(Token::Whitespace), &mut tokenizer).expect("failed to forecast");
        assert_eq!(
            forecast_result,
            Forecasting {
                start: Token::Whitespace,
                end: Token::Whitespace,
                data: b"id,",
            }
        )
    }
}
