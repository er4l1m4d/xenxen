use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::db::AggregateStats;

// ---------------------------------------------------------------------------
// Balance status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceStatus {
    /// Remaining > $10 — comfortable
    Healthy,
    /// $5 < remaining <= $10 — approaching threshold
    Warning,
    /// Remaining <= $5 — auto-reload will trigger
    Critical,
    /// Remaining <= 0 — depleted
    Depleted,
}

impl BalanceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Healthy => "Healthy",
            Self::Warning => "Low",
            Self::Critical => "Critical",
            Self::Depleted => "Depleted",
        }
    }

    pub fn from_remaining(remaining: f64, threshold: f64) -> Self {
        if remaining <= 0.0 {
            Self::Depleted
        } else if remaining <= threshold {
            Self::Critical
        } else if remaining <= threshold * 2.0 {
            Self::Warning
        } else {
            Self::Healthy
        }
    }
}

// ---------------------------------------------------------------------------
// BalanceTracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSnapshot {
    pub total_deposited: f64,
    pub cumulative_spend: f64,
    pub remaining: f64,
    pub status: BalanceStatus,
    pub burn_rate_daily: f64,
    pub days_until_empty: Option<f64>,
    pub sessions_per_day: f64,
    pub auto_reload_threshold: f64,
    pub auto_reload_amount: f64,
    pub will_auto_reload: bool,
}

pub struct BalanceTracker {
    config: Config,
}

impl BalanceTracker {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Compute the full balance snapshot from aggregate stats.
    pub fn snapshot(&self, stats: &AggregateStats) -> BalanceSnapshot {
        let total_deposited = self.config.total_deposited();
        let cumulative_spend = stats.total_cost;
        let remaining = total_deposited - cumulative_spend;
        let status = BalanceStatus::from_remaining(remaining, self.config.auto_reload_threshold);

        // Compute daily burn rate from the daily breakdown
        let (burn_rate_daily, sessions_per_day) = if !stats.daily.is_empty() {
            let total_daily_cost: f64 = stats.daily.iter().map(|d| d.cost).sum();
            let total_daily_sessions: usize = stats.daily.iter().map(|d| d.sessions).sum();
            let day_count = stats.daily.len() as f64;
            if day_count > 0.0 {
                (
                    total_daily_cost / day_count,
                    total_daily_sessions as f64 / day_count,
                )
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        // Project days until empty
        let days_until_empty = if burn_rate_daily > 0.0 && remaining > 0.0 {
            Some(remaining / burn_rate_daily)
        } else {
            None
        };

        // Auto-reload check
        let will_auto_reload = remaining <= self.config.auto_reload_threshold
            && self.config.auto_reload_amount > 0.0;

        BalanceSnapshot {
            total_deposited,
            cumulative_spend,
            remaining,
            status,
            burn_rate_daily,
            days_until_empty,
            sessions_per_day,
            auto_reload_threshold: self.config.auto_reload_threshold,
            auto_reload_amount: self.config.auto_reload_amount,
            will_auto_reload,
        }
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

pub fn format_days_until_empty(days: Option<f64>) -> String {
    match days {
        None => "N/A (no spend data)".to_string(),
        Some(d) if d > 365.0 => format!("{:.0}+ years", d / 365.0),
        Some(d) if d > 30.0 => format!("{:.0} months", d / 30.0),
        Some(d) if d > 1.0 => format!("{:.1} days", d),
        Some(d) if d > 0.0 => format!("{:.1} hours", d * 24.0),
        Some(_) => "Depleted".to_string(),
    }
}

pub fn format_burn_rate(rate: f64) -> String {
    if rate <= 0.0 {
        "$0.00/day".to_string()
    } else if rate < 0.01 {
        format!("${:.4}/day", rate)
    } else {
        format!("${:.2}/day", rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Topup;

    fn test_config() -> Config {
        Config {
            initial_balance: 20.0,
            topups: vec![
                Topup { date: "2026-06-01".into(), amount: 20.0, note: None },
                Topup { date: "2026-06-15".into(), amount: 20.0, note: None },
            ],
            auto_reload_threshold: 5.0,
            auto_reload_amount: 20.0,
            ..Default::default()
        }
    }

    fn test_stats(cost: f64, daily_costs: Vec<f64>) -> AggregateStats {
        let daily = daily_costs
            .iter()
            .enumerate()
            .map(|(i, &c)| crate::db::DailySummary {
                date: format!("2026-06-{:02}", i + 1),
                cost: c,
                sessions: 1,
                tokens_input: 0,
                tokens_output: 0,
                tokens_reasoning: 0,
                tokens_cache_read: 0,
                tokens_cache_write: 0,
            })
            .collect();

        AggregateStats {
            total_sessions: daily_costs.len(),
            total_cost: cost,
            total_tokens_input: 0,
            total_tokens_output: 0,
            total_tokens_reasoning: 0,
            total_tokens_cache_read: 0,
            total_tokens_cache_write: 0,
            daily,
            by_model: vec![],
            by_project: vec![],
            top_tools: vec![],
        }
    }

    #[test]
    fn test_balance_status_healthy() {
        assert_eq!(BalanceStatus::from_remaining(15.0, 5.0), BalanceStatus::Healthy);
    }

    #[test]
    fn test_balance_status_warning() {
        assert_eq!(BalanceStatus::from_remaining(8.0, 5.0), BalanceStatus::Warning);
    }

    #[test]
    fn test_balance_status_critical() {
        assert_eq!(BalanceStatus::from_remaining(3.0, 5.0), BalanceStatus::Critical);
    }

    #[test]
    fn test_balance_status_depleted() {
        assert_eq!(BalanceStatus::from_remaining(0.0, 5.0), BalanceStatus::Depleted);
        assert_eq!(BalanceStatus::from_remaining(-1.0, 5.0), BalanceStatus::Depleted);
    }

    #[test]
    fn test_snapshot_computation() {
        let tracker = BalanceTracker::new(test_config());
        // $25 spent over 10 days
        let daily_costs: Vec<f64> = (0..10).map(|_| 2.5).collect();
        let stats = test_stats(25.0, daily_costs);

        let snap = tracker.snapshot(&stats);
        assert_eq!(snap.total_deposited, 60.0);
        assert_eq!(snap.cumulative_spend, 25.0);
        assert_eq!(snap.remaining, 35.0);
        assert_eq!(snap.status, BalanceStatus::Healthy);
        assert!((snap.burn_rate_daily - 2.5).abs() < 0.01);
        assert!(snap.days_until_empty.is_some());
        assert!((snap.days_until_empty.unwrap() - 14.0).abs() < 0.1);
    }

    #[test]
    fn test_snapshot_auto_reload() {
        let tracker = BalanceTracker::new(test_config());
        // Spending 56 of 60 → $4 remaining → below threshold
        let stats = test_stats(56.0, vec![5.6; 10]);
        let snap = tracker.snapshot(&stats);
        assert_eq!(snap.status, BalanceStatus::Critical);
        assert!(snap.will_auto_reload);
    }

    #[test]
    fn test_format_days() {
        assert_eq!(format_days_until_empty(None), "N/A (no spend data)");
        assert!(format_days_until_empty(Some(0.5)).contains("hours"));
        assert!(format_days_until_empty(Some(5.0)).contains("days"));
        assert!(format_days_until_empty(Some(45.0)).contains("months"));
    }
}
