use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Thresholds {
    pub rx_kb_per_sec: u64,
    pub tx_kb_per_sec: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Sampling {
    pub period_secs: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub telegram_token: String,
    pub telegram_chat_id: String,
    pub log_paths: Vec<String>,
    pub thresholds: Thresholds,
    pub sampling: Sampling,
}

pub fn load_config(path: &str) -> Config {
    let content = fs::read_to_string(path).expect("Failed to read config file");
    toml::from_str(&content).expect("Failed to parse config")
}
