use crate::parser::commands::Command;
use crate::parser::parse_command;
use clap::Parser;
use futures::StreamExt;
use pusu_client_lib::errors::Result;
use pusu_client_lib::PusuClient;
use std::io::{stdout, Write};
use std::sync::atomic::AtomicBool;
use tokio::select;
use tokio::sync::mpsc::Receiver;

static VERSION: &str = env!("CARGO_PKG_VERSION");
static NAME: &str = env!("CARGO_PKG_NAME");

static SUBSCRIPTION_MODE: AtomicBool = AtomicBool::new(false);

mod cli;
mod parser;

mod errors;

pub async fn run() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = cli::Cli::parse();

    println!("{} v{}", NAME, VERSION);

    println!("Connecting to {}:{}...", cli.host, cli.port);

    let mut client = PusuClient::new();
    client.connect(&cli.host, cli.port).await?;

    let (dropper_tx, mut dropper_rx) = tokio::sync::mpsc::channel(1000);

    tokio::spawn(async move {
        loop {
            tokio::signal::ctrl_c().await.unwrap();
            if SUBSCRIPTION_MODE.load(std::sync::atomic::Ordering::Acquire) {
                dropper_tx.send(()).await.unwrap();
            } else {
                println!("\r\nExiting...");
                std::process::exit(0);
            }
        }
    });

    loop {
        stdout().flush()?;
        print!("> ");
        stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();
        let command = parse_command(input);
        if let Ok(command) = command {
            execute_command(command, &mut client, &mut dropper_rx).await?;
        } else {
            println!("Invalid command: {}", input);
        }
    }
}

pub async fn execute_command(
    command: Command<'_>,
    client: &mut PusuClient,
    dropper_rx: &mut Receiver<()>,
) -> Result<()> {
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
            let mut stream = client.receive().await?.boxed();
            SUBSCRIPTION_MODE.store(true, std::sync::atomic::Ordering::Release);

            println!("Entering subscription mode...(Press Ctrl+C to Exit)");
            stdout().flush()?;

            loop {
                select! {
                    _ = dropper_rx.recv() => {
                        SUBSCRIPTION_MODE.store(false, std::sync::atomic::Ordering::Release);
                        println!("\r\nExiting subscription mode...");
                        break;
                    }
                    message = stream.next() => {
                        match message {
                            None => {break}
                            Some(message) => {
                                println!("Received message: {}", String::from_utf8_lossy(&message));
                            }
                        }
                    }
                }
            }
        }
    };
    Ok(())
}
