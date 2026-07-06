use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Daily token limit for free tier OpenCode Zen.
pub const DAILY_PART_LIMIT: usize = 5000;

/// Configuration file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Dashboard refresh interval in seconds
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,

    /// Daily token limit (input + output)
    #[serde(default = "default_daily_limit_tokens")]
    pub daily_limit_tokens: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_secs: 5,
            daily_limit_tokens: 5_000_000,
        }
    }
}

fn default_refresh_interval() -> u64 {
    5
}

fn default_daily_limit_tokens() -> i64 {
    5_000_000
}

impl Config {
    /// Get the path to the config file: ~/.config/xenxen/config.toml
    pub fn config_path() -> Option<PathBuf> {
        let config_dir = dirs::config_dir()?;
        Some(config_dir.join("xenxen").join("config.toml"))
    }

    /// Load config from disk, or return default if not found.
    pub fn load() -> Self {
        match Self::config_path() {
            Some(path) if path.exists() => {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        match toml::from_str(&content) {
                            Ok(config) => config,
                            Err(e) => {
                                eprintln!("Warning: Config parse error at {}: {}", path.display(), e);
                                eprintln!("  Using defaults. Fix the config or delete it to reset.");
                                Self::default()
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Could not read config at {}: {}", path.display(), e);
                        Self::default()
                    }
                }
            }
            _ => Self::default(),
        }
    }

    /// Save config to disk.
    #[allow(dead_code)]
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path().ok_or("Could not determine config directory (check $XDG_CONFIG_HOME)")?;

        // Create parent directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.refresh_interval_secs, 5);
        assert_eq!(config.daily_limit_tokens, 5_000_000);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = Config { refresh_interval_secs: 10, daily_limit_tokens: 500_000 };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.refresh_interval_secs, 10);
        assert_eq!(parsed.daily_limit_tokens, 500_000);
    }

    #[test]
    fn test_config_missing_fields_fallback() {
        let toml_str = "";
        let parsed: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(parsed.refresh_interval_secs, 5);
        assert_eq!(parsed.daily_limit_tokens, 5_000_000);
    }
}
