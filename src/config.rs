use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::warn;

const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_phone_ip")]
    pub phone_ip: String,

    #[serde(default = "default_control_port")]
    pub control_port: u16,

    #[serde(default = "default_media_port")]
    pub media_port: u16,

    #[serde(default = "default_source_name")]
    pub source_name: String,

    #[serde(default = "default_auto_connect")]
    pub auto_connect: bool,

    #[serde(default = "default_theme")]
    pub theme: Theme,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            phone_ip: default_phone_ip(),
            control_port: default_control_port(),
            media_port: default_media_port(),
            source_name: default_source_name(),
            auto_connect: default_auto_connect(),
            theme: default_theme(),
        }
    }
}

fn default_phone_ip() -> String { "192.168.1.105".into() }
fn default_control_port() -> u16 { 8125 }
fn default_media_port() -> u16 { 49152 }
fn default_source_name() -> String { "yumic_source".into() }
fn default_auto_connect() -> bool { false }
fn default_theme() -> Theme { Theme::System }

impl Config {
    fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("yumic")
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join(CONFIG_FILE)
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    warn!("Failed to parse config at {}: {}. Using defaults.", path.display(), e);
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(Self::config_path(), content)?;
        Ok(())
    }
}
