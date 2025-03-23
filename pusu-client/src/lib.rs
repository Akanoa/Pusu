use crate::parser::commands::Command;
use crate::parser::parse_command;
use clap::Parser;
use futures::StreamExt;
use pusu_client_lib::errors::Result;
use pusu_client_lib::PusuClient;
use std::io::Write;

mod cli;
mod parser;

mod errors;

pub async fn run() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = cli::Cli::parse();

    println!("Connecting to {}:{}", cli.host, cli.port);

    let mut client = PusuClient::new();
    client.connect(&cli.host, cli.port).await?;

    println!("Connected to {}:{}", cli.host, cli.port);

    loop {
        print!("> ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();
        let command = parse_command(input);
        if let Ok(command) = command {
            execute_command(command, &mut client).await?;
        }
    }
}

pub async fn execute_command(command: Command<'_>, client: &mut PusuClient) -> Result<()> {
    match command {
        Command::Auth(parser::commands::auth::Auth { token }) => {
            println!("Authenticating...");
            client.authenticate(token).await?;
        }
        Command::Publish(parser::commands::publish::Publish { channel, message }) => {
            println!("Publishing message to channel {}...", channel);
            client.publish(channel, message).await?;
        }
        Command::Subscribe(parser::commands::subscribe::Subscribe { channel }) => {
            println!("Subscribing to channel {}...", channel);
            client.subscribe(channel).await?;
        }
        Command::Unsubscribe(parser::commands::unsubscribe::Unsubscribe { channel }) => {
            println!("Unsubscribing from channel {}...", channel);
            client.unsubscribe(channel).await?;
        }
        Command::Consume(_) => {
            let mut stream = client.receive().await?;
            while let Some(message) = stream.next().await {
                println!("Received message: {}", String::from_utf8_lossy(&message));
            }
        }
    };
    Ok(())
}
