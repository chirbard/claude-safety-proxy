use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_chunk_chars")]
    pub max_chunk_chars: usize,
}

fn default_port() -> u16 { 8080 }
fn default_ollama_url() -> String { "http://localhost:11434".into() }
fn default_model() -> String { "llama3.2:3b".into() }
fn default_max_chunk_chars() -> usize { 4000 }

impl Config {
    pub fn load() -> Result<Self> {
        let path = std::env::var("CONFIG").unwrap_or_else(|_| "config.toml".into());
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("reading config from {path}"))?;
        let config: Config = toml::from_str(&raw)
            .with_context(|| format!("parsing config from {path}"))?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: default_port(),
            ollama_url: default_ollama_url(),
            model: default_model(),
            max_chunk_chars: default_max_chunk_chars(),
        }
    }
}
