use crate::errors::{PusuClientError, Result};
use crate::parser::commands::Command;
use crate::parser::scanner::Scanner;

mod acceptor;
pub mod commands;
pub mod errors;
mod forecaster;
mod recognizer;
mod scanner;
mod token;
mod until_end;
mod until_token;
mod visitor;
mod whitespaces;

pub fn parse_command(command: &str) -> Result<Command> {
    let mut scanner = Scanner::new(command.as_bytes());
    let command = scanner.visit::<Command>().map_err(|err| match err {
        errors::ParseError::UnexpectedToken => PusuClientError::UnknownCommand,
        err => err.into(),
    })?;

    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_command() {
        let command = "SUBSCRIBE my::channel";
        let result = parse_command(command).expect("Unable to parse command");
        assert_eq!(
            result,
            Command::Subscribe(commands::subscribe::Subscribe {
                channel: "my::channel"
            })
        );

        let command = "UNSUBSCRIBE my::channel";
        let result = parse_command(command).expect("Unable to parse command");
        assert_eq!(
            result,
            Command::Unsubscribe(commands::unsubscribe::Unsubscribe {
                channel: "my::channel"
            })
        );

        let command = "PUBLISH my::channel Hello world";
        let result = parse_command(command).expect("Unable to parse command");
        assert_eq!(
            result,
            Command::Publish(commands::publish::Publish {
                channel: "my::channel",
                message: "Hello world".as_bytes()
            })
        );

        let command = "CONSUME";
        let result = parse_command(command).expect("Unable to parse command");
        assert_eq!(result, Command::Consume(commands::consume::Consume));

        let command = "Bad command";
        let result = parse_command(command);
        assert!(result.is_err());
    }
}
