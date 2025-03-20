use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub configuration: PathBuf,
}
