mod balance;
mod config;
mod db;
mod tui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xenxen", version = "0.1.0", about = "Track your OpenCode Zen usage and balance")]
struct Cli {
    /// Path to OpenCode database (overrides auto-detection)
    #[arg(long, env = "OPENCODE_DB")]
    db_path: Option<String>,

    /// Mini mode: compact 5-line status (for status bars)
    #[arg(long, global = true)]
    mini: bool,

    /// Export stats to CSV file
    #[arg(long)]
    export_csv: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the interactive TUI dashboard
    Dashboard,
    /// Show stats as plain text (non-TUI)
    Stats {
        /// Show stats for the last N days
        #[arg(short, long)]
        days: Option<u64>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Configure balance tracking
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Set your initial Zen balance
    SetInitialBalance {
        /// Balance amount in USD
        amount: f64,
    },
    /// Record a top-up
    AddTopup {
        /// Amount in USD
        amount: f64,
        /// Date (YYYY-MM-DD), defaults to today
        #[arg(short, long)]
        date: Option<String>,
        /// Optional note
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Show current config
    Show,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // --mini is only valid with Dashboard or no command
    let is_mini = cli.mini && matches!(cli.command, Some(Commands::Dashboard) | None);

    // Handle --export-csv early
    if let Some(ref csv_path) = cli.export_csv {
        let db_path = resolve_db_path(&cli.db_path)?;
        let conn = db::open_database(&db_path)?;
        let stats = db::aggregate_stats(&conn, None)?;
        let cfg = config::Config::load();
        let tracker = balance::BalanceTracker::new(cfg);
        let snap = tracker.snapshot(&stats);
        match export_csv(csv_path, &stats, &snap) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: Could not export CSV: {}", e);
                return Err(e);
            }
        }
        return Ok(());
    }

    match cli.command {
        Some(Commands::Config { action }) => {
            let mut config = config::Config::load();
            match action {
                ConfigAction::SetInitialBalance { amount } => {
                    config.initial_balance = amount;
                    config.save()?;
                    println!("Initial balance set to ${:.2}", amount);
                }
                ConfigAction::AddTopup { amount, date, note } => {
                    let topup_date = date.unwrap_or_else(|| {
                        chrono::Local::now().format("%Y-%m-%d").to_string()
                    });
                    config.topups.push(config::Topup {
                        date: topup_date,
                        amount,
                        note,
                    });
                    config.save()?;
                    println!("Recorded top-up of ${:.2}", amount);
                    println!("Total deposited: ${:.2}", config.total_deposited());
                }
                ConfigAction::Show => {
                    println!("Config file: {}", config::Config::config_path().unwrap().display());
                    println!("Initial balance: ${:.2}", config.initial_balance);
                    println!("Top-ups: {}", config.topups.len());
                    for t in &config.topups {
                        println!("  {} - ${:.2}", t.date, t.amount);
                    }
                    println!("Total deposited: ${:.2}", config.total_deposited());
                    println!("Auto-reload threshold: ${:.2}", config.auto_reload_threshold);
                    println!("Auto-reload amount: ${:.2}", config.auto_reload_amount);
                }
            }
        }
        Some(Commands::Stats { days, json }) => {
            let db_path = resolve_db_path(&cli.db_path)?;
            let conn = db::open_database(&db_path)?;
            let stats = db::aggregate_stats(&conn, days)?;
            let cfg = config::Config::load();
            let tracker = balance::BalanceTracker::new(cfg);
            let snap = tracker.snapshot(&stats);

            if json {
                println!("{}", serde_json::to_string_pretty(&snap)?);
            } else {
                println!("=== Balance ===");
                println!("  Deposited:     {}", db::format_cost(snap.total_deposited));
                println!("  Spent:         {}", db::format_cost(snap.cumulative_spend));
                println!("  Remaining:     {}", db::format_cost(snap.remaining));
                println!("  Status:        {}", snap.status.label());
                println!("  Burn Rate:     {}", balance::format_burn_rate(snap.burn_rate_daily));
                println!("  Days Left:     {}", balance::format_days_until_empty(snap.days_until_empty));
                if snap.will_auto_reload {
                    println!("  Auto-reload:   {} when below {}", db::format_cost(snap.auto_reload_amount), db::format_cost(snap.auto_reload_threshold));
                }

                println!("\n=== Usage ===");
                println!("  Sessions:      {}", stats.total_sessions);
                println!("  Input:         {} tokens", db::format_tokens(stats.total_tokens_input));
                println!("  Output:        {} tokens", db::format_tokens(stats.total_tokens_output));
                println!("  Reasoning:     {} tokens", db::format_tokens(stats.total_tokens_reasoning));
                println!("  Cache Read:    {} tokens", db::format_tokens(stats.total_tokens_cache_read));

                if !stats.by_model.is_empty() {
                    println!("\n=== Models ===");
                    for m in &stats.by_model {
                        println!("  {:<25} {:>8}  {:>6} sessions  in: {}  out: {}",
                            m.model_id, db::format_cost(m.cost), m.sessions,
                            db::format_tokens(m.tokens_input), db::format_tokens(m.tokens_output));
                    }
                }
                if !stats.by_project.is_empty() {
                    println!("\n=== Projects ===");
                    for p in &stats.by_project {
                        let name = if p.project_name.len() > 30 {
                            format!("{}…", &p.project_name[..29])
                        } else {
                            p.project_name.clone()
                        };
                        println!("  {:<30} {:>8}  {:>6} sessions",
                            name, db::format_cost(p.cost), p.sessions);
                    }
                }
                if !stats.top_tools.is_empty() {
                    println!("\n=== Top Tools ===");
                    for t in stats.top_tools.iter().take(10) {
                        println!("  {:<25} {:>6} calls", t.tool_name, t.count);
                    }
                }
                if days.is_some() {
                    println!("\n(filtered to last {} days)", days.unwrap());
                }
            }
        }
        Some(Commands::Dashboard) | None => {
            let db_path = resolve_db_path(&cli.db_path)?;
            let conn = db::open_database(&db_path)?;
            let config = config::Config::load();
            if is_mini {
                render_mini(&conn, &config)?;
            } else {
                let app = tui::App::new(conn, config);
                let terminal = ratatui::init();
                let result = tui::run(terminal, app);
                ratatui::restore();
                result?;
            }
        }
    }

    Ok(())
}

fn render_mini(conn: &rusqlite::Connection, config: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
    let stats = db::aggregate_stats(conn, None)?;
    let tracker = balance::BalanceTracker::new(config.clone());
    let snap = tracker.snapshot(&stats);

    let status = match snap.status {
        balance::BalanceStatus::Healthy => "OK",
        balance::BalanceStatus::Warning => "LOW",
        balance::BalanceStatus::Critical => "!!",
        balance::BalanceStatus::Depleted => "EMPTY",
    };

    let bar_width = 15;
    let fraction = if snap.total_deposited > 0.0 {
        (snap.remaining / snap.total_deposited).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (fraction * bar_width as f64) as usize;
    let empty = bar_width as usize - filled;
    let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

    println!("{} {} {} ({:.0}%)",
        db::format_cost(snap.remaining),
        status,
        bar,
        fraction * 100.0,
    );
    println!("sessions: {} | burn: {} | left: {}",
        stats.total_sessions,
        balance::format_burn_rate(snap.burn_rate_daily),
        balance::format_days_until_empty(snap.days_until_empty),
    );
    if let Some(last) = stats.daily.first() {
        println!("last day: {} sessions, {}",
            last.sessions,
            db::format_cost(last.cost),
        );
    } else {
        println!("no activity yet");
    }
    Ok(())
}

fn export_csv(path: &str, stats: &db::AggregateStats, snap: &balance::BalanceSnapshot) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = csv::Writer::from_path(path)?;

    // Summary row
    wtr.write_record(&["type", "key", "value"])?;
    wtr.write_record(&["summary", "remaining", &format!("{:.4}", snap.remaining)])?;
    wtr.write_record(&["summary", "deposited", &format!("{:.4}", snap.total_deposited)])?;
    wtr.write_record(&["summary", "spent", &format!("{:.4}", snap.cumulative_spend)])?;
    wtr.write_record(&["summary", "sessions", &stats.total_sessions.to_string()])?;
    wtr.write_record(&["summary", "tokens_input", &stats.total_tokens_input.to_string()])?;
    wtr.write_record(&["summary", "tokens_output", &stats.total_tokens_output.to_string()])?;

    // Daily breakdown
    for d in &stats.daily {
        wtr.write_record(&["daily", &d.date, &format!("{:.4}", d.cost)])?;
    }

    // Model breakdown
    for m in &stats.by_model {
        wtr.write_record(&["model", &m.model_id, &format!("{:.4}", m.cost)])?;
    }

    // Tool usage
    for t in &stats.top_tools {
        wtr.write_record(&["tool", &t.tool_name, &t.count.to_string()])?;
    }

    wtr.flush()?;
    println!("Exported to {}", path);
    Ok(())
}

fn resolve_db_path(db_path: &Option<String>) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    match db_path {
        Some(p) => {
            let path = std::path::PathBuf::from(p);
            if !path.exists() {
                eprintln!("Error: Database file not found: {}", path.display());
                eprintln!("  Use 'xenxen config show' to check your setup, or specify a valid --db-path.");
                return Err("Database file not found".into());
            }
            Ok(path)
        }
        None => {
            let found = db::find_database();
            match found {
                Some(path) => Ok(path),
                None => {
                    eprintln!("Error: OpenCode database not found.");
                    eprintln!("  Searched in:");
                    eprintln!("    - $OPENCODE_DB env var");
                    #[cfg(target_os = "windows")]
                    {
                        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
                            eprintln!("    - {}\\opencode\\opencode.db", app_data);
                        }
                    }
                    if let Some(home) = dirs::home_dir() {
                        eprintln!("    - {}\\.local\\share\\opencode\\opencode.db", home.display());
                    }
                    if let Some(data_dir) = dirs::data_local_dir() {
                        eprintln!("    - {}\\opencode\\opencode.db", data_dir.display());
                    }
                    eprintln!();
                    eprintln!("  Fix: Run 'xenxen --db-path <path>' or set $OPENCODE_DB.");
                    Err("OpenCode database not found. Use --db-path to specify.".into())
                }
            }
        }
    }
}
