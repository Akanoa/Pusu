use clap::Parser;
use pusu_client_lib::errors::Result;
use pusu_client_lib::PusuClient;

mod cli;
mod parser;

mod errors;

pub async fn run() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = cli::Cli::parse();

    let mut client = PusuClient::new();
    client.connect(&cli.host, cli.port).await?;

    println!("Hello, world!");
    Ok(())
}
