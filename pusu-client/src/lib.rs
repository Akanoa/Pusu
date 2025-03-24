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

/// Entry point of the Pusu client application.
///
/// This asynchronous function initializes the environment, parses the command line arguments,
/// connects to the server, and enters an interactive loop to handle user input and commands.
///
/// Key features include:
/// - Setting up the tracing subscriber for logging.
/// - Parsing command line arguments to configure server host and port.
/// - Establishing a connection with the server using the provided `PusuClient`.
/// - Handling user input to execute commands or gracefully exit the application.
/// - Listening for Ctrl+C interruptions to manage subscription mode or terminate the program.
///
/// # Returns
///
/// A `Result` that indicates whether the application ran successfully. Errors may occur
/// during initialization, connection, or command execution.
///
/// # Behavior
///
/// The interactive loop:
/// - Prompts the user and processes their input.
/// - Parses the user's input into a command.
/// - Executes the command using the `execute_command` function.
/// - Handles invalid or malformed commands by displaying an error message.
///
/// Subscription mode is supported, allowing the user to consume messages
/// from the server. Pressing Ctrl+C while in subscription mode exits it, but
/// if not in subscription mode, it terminates the application.
///
/// # Errors
///
/// - Returns an error if connection to the server fails.
/// - Returns an error if input from stdin cannot be read.
/// - Errors may also originate from underlying command executions or the client library.
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

/// Executes the specified command by interacting with the PusuClient instance
/// and handling the provided command logic.
///
/// # Arguments
///
/// * `command` - The `Command` to be executed, which encapsulates the user's input.
/// * `client` - A mutable reference to the `PusuClient` instance to execute the command on.
/// * `dropper_rx` - A mutable reference to a `Receiver` used for inter-task communication
///   to manage subscription mode and handle user interruptions.
///
/// # Returns
///
/// A `Result` indicating the success (`Ok`) or failure (`Err`) of the command execution.
/// Possible errors originate from underlying client operations or invalid commands.
///
/// # Behavior
///
/// This function matches the command type to execute the appropriate action:
/// - Handles user authentication.
/// - Manages publishing messages to channels.
/// - Manages subscribing and unsubscribing to/from channels.
/// - Supports consuming messages in subscription mode.
/// - Provides help information.
/// - Allows for graceful client exit.
///
/// When in "subscription mode," the function listens for incoming messages and
/// handles the interruption signal (e.g., Ctrl+C) to exit the subscription mode
/// gracefully.
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
        Command::Exit(_) => {
            std::process::exit(0);
        }
        Command::Help(_) => {
            println!("Available commands:");
            println!("  AUTH <token>                - Authenticate with the server using a token");
            println!("  PUBLISH <channel> <message> - Publish a message to a specified channel");
            println!("  SUBSCRIBE <channel>         - Subscribe to a specified channel");
            println!("  UNSUBSCRIBE <channel>       - Unsubscribe from a specified channel");
            println!(
                "  CONSUME                     - Consume messages from the subscribed channel(s)"
            );
            println!("  HELP                        - Show this help message");
            println!("  STOP                        - Stop the client and exit");
            println!("  EXIT                        - Exit the client");
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
