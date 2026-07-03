use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single top-up record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topup {
    /// Date as YYYY-MM-DD
    pub date: String,
    /// Amount in USD
    pub amount: f64,
    /// Optional note
    pub note: Option<String>,
}

/// Configuration file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Starting balance when you first set up tracking
    #[serde(default)]
    pub initial_balance: f64,

    /// List of top-ups
    #[serde(default)]
    pub topups: Vec<Topup>,

    /// Auto-reload threshold in USD (default: $5.00)
    #[serde(default = "default_auto_reload_threshold")]
    pub auto_reload_threshold: f64,

    /// Auto-reload amount in USD (default: $20.00)
    #[serde(default = "default_auto_reload_amount")]
    pub auto_reload_amount: f64,

    /// Watch mode refresh interval in seconds
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            initial_balance: 0.0,
            topups: Vec::new(),
            auto_reload_threshold: 5.0,
            auto_reload_amount: 20.0,
            refresh_interval_secs: 5,
        }
    }
}

fn default_auto_reload_threshold() -> f64 {
    5.0
}

fn default_auto_reload_amount() -> f64 {
    20.0
}

fn default_refresh_interval() -> u64 {
    5
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

    /// Compute total deposited (initial balance + all top-ups).
    pub fn total_deposited(&self) -> f64 {
        let topup_total: f64 = self.topups.iter().map(|t| t.amount).sum();
        self.initial_balance + topup_total
    }

    /// Compute remaining balance given cumulative spend.
    pub fn remaining_balance(&self, cumulative_spend: f64) -> f64 {
        self.total_deposited() - cumulative_spend
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.initial_balance, 0.0);
        assert_eq!(config.topups.len(), 0);
        assert_eq!(config.auto_reload_threshold, 5.0);
    }

    #[test]
    fn test_total_deposited() {
        let config = Config {
            initial_balance: 20.0,
            topups: vec![
                Topup { date: "2026-01-01".into(), amount: 20.0, note: None },
                Topup { date: "2026-02-01".into(), amount: 20.0, note: None },
            ],
            ..Default::default()
        };
        assert_eq!(config.total_deposited(), 60.0);
    }

    #[test]
    fn test_remaining_balance() {
        let config = Config {
            initial_balance: 20.0,
            topups: vec![Topup { date: "2026-01-01".into(), amount: 20.0, note: None }],
            ..Default::default()
        };
        assert_eq!(config.remaining_balance(15.0), 25.0);
    }
}
