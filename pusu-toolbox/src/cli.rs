use clap::Subcommand;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub commnd: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new biscuit
    Biscuit(BiscuitArgs),
}

#[derive(Debug, clap::Args)]
pub struct BiscuitArgs {
    /// Tenant of the token to create
    pub tenant: String,
    /// Optional hex private key representation (Ed25519)
    #[clap(long)]
    pub key: Option<String>,
}
