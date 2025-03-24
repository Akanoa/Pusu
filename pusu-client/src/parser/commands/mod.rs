use crate::parser::acceptor::Acceptor;
use crate::parser::errors::{ParseError, ParseResult};
use crate::parser::scanner::Scanner;
use crate::parser::visitor::Visitable;

pub mod auth;
pub mod consume;
pub mod exit;
pub mod help;
pub mod publish;
pub mod quit;
pub mod subscribe;
pub mod unsubscribe;

#[derive(Debug, PartialEq)]
pub enum Command<'a> {
    Auth(auth::Auth<'a>),
    Publish(publish::Publish<'a>),
    Subscribe(subscribe::Subscribe<'a>),
    Unsubscribe(unsubscribe::Unsubscribe<'a>),
    Consume(consume::Consume),
    Help(help::Help),
    Exit(exit::Exit),
    Quit(quit::Quit),
}

impl<'a> Visitable<'a, u8> for Command<'a> {
    fn accept(scanner: &mut Scanner<'a, u8>) -> ParseResult<Self> {
        Acceptor::new(scanner)
            .try_or(|command| Ok(Command::Help(command)))?
            .try_or(|command| Ok(Command::Exit(command)))?
            .try_or(|command| Ok(Command::Quit(command)))?
            .try_or(|command| Ok(Command::Consume(command)))?
            .try_or(|command| Ok(Command::Auth(command)))?
            .try_or(|command| Ok(Command::Publish(command)))?
            .try_or(|command| Ok(Command::Unsubscribe(command)))?
            .try_or(|command| Ok(Command::Subscribe(command)))?
            .finish()
            .ok_or(ParseError::UnexpectedToken)
    }
}
