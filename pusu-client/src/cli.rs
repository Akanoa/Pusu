#[derive(Debug, clap::Parser)]
pub struct Cli {
    /// Pusu server hostname
    pub host: String,
    /// Pusu server port
    pub port: u16,
}
