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

pub struct Forecaster<'a, 'b, T, S> {
    scanner: &'b mut Scanner<'a, T>,
    forcastables: Vec<Box<dyn Forecast<'a, T, S>>>,
}

impl<'a, 'b, T, S> Forecaster<'a, 'b, T, S> {
    pub fn new(scanner: &'b mut Scanner<'a, T>) -> Self {
        Self {
            scanner,
            forcastables: vec![],
        }
    }
}

impl<'a, T, S> Forecaster<'a, '_, T, S> {
    /// Add new [Forecast] element to the forecasting pool
    pub fn add_forecastable<F: Forecast<'a, T, S> + 'static>(mut self, forecastable: F) -> Self {
        self.forcastables.push(Box::new(forecastable));
        self
    }

    /// Run the [Forecast] pool, find the minimal group
    pub fn forecast(self) -> parser::errors::ParseResult<Option<Forecasting<'a, T, S>>> {
        let mut result = None;
        // on boucle sur les possibilités de prédictions
        for forecastable in self.forcastables.into_iter() {
            // on tente de prédire l'élément
            match forecastable.forecast(self.scanner)? {
                // si l'on a trouvé quelque chose
                ForecastResult::Found {
                    start,
                    end,
                    end_slice,
                } => {
                    // on récupère le groupe prédit
                    let data = &self.scanner.remaining()[..end_slice];
                    let new_forecast = Forecasting { start, end, data };
                    match &result {
                        // si l'on n'a encore rien prédit du tout
                        None => {
                            // le groupe trouvé devient le résultat
                            result = Some(new_forecast);
                        }
                        // s'il y a déjà une prédiction
                        Some(min_forecast) => {
                            // on compare la taille du groupe trouvé par rapport
                            // à celui déjà trouvé
                            if new_forecast.data.len() < min_forecast.data.len() {
                                // il devient alors le nouveau groupe prédit
                                result = Some(new_forecast);
                            }
                        }
                    }
                }
                // si la prédiction échoue, on ne fait rien
                ForecastResult::NotFound => {}
            }
        }
        Ok(result)
    }
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
