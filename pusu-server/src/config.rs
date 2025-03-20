use figment::providers::{Format, Toml};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct PusuServerConfig {
    pub port: u16,
    pub host: String,
    pub cluster_file: String,
    pub public_key: String,
    pub api_version: i32,
}

impl PusuServerConfig {
    pub fn new(configuration_file: &PathBuf) -> crate::errors::Result<Self> {
        Ok(figment::Figment::new()
            .merge(Toml::file(configuration_file))
            .extract()?)
    }
}
