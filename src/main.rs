mod config;
mod db;
mod tui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xenxen", version = "0.1.0", about = "Track your OpenCode Zen usage")]
struct Cli {
    /// Path to OpenCode database (overrides auto-detection)
    #[arg(long, env = "OPENCODE_DB")]
    db_path: Option<String>,

    /// Mini mode: compact status line (for status bars)
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
    /// Show current config
    Config,
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
        match export_csv(csv_path, &stats) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: Could not export CSV: {}", e);
                return Err(e);
            }
        }
        return Ok(());
    }

    match cli.command {
        Some(Commands::Config) => {
            let config = config::Config::load();
            println!("Config file: {}", config::Config::config_path().unwrap().display());
            println!("Refresh interval: {}s", config.refresh_interval_secs);
            println!("Daily token limit: {}", db::format_tokens(config.daily_limit_tokens));
            println!("Daily request limit: {}", config::DAILY_PART_LIMIT);
        }
        Some(Commands::Stats { days, json }) => {
            let db_path = resolve_db_path(&cli.db_path)?;
            let conn = db::open_database(&db_path)?;
            let stats = db::aggregate_stats(&conn, days)?;
            let today_parts = db::todays_part_count(&conn).unwrap_or(0);
            let cfg = config::Config::load();

            if json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!("=== Today ===");
                println!("  Requests:     {} / {}", today_parts, config::DAILY_PART_LIMIT);
                let today_tokens = stats.daily.first().map(|d| d.tokens_input + d.tokens_output).unwrap_or(0);
                println!("  Tokens:       {} / {}", db::format_tokens(today_tokens), db::format_tokens(cfg.daily_limit_tokens));
                let today_sessions = stats.daily.first().map(|d| d.sessions).unwrap_or(0);
                println!("  Sessions:     {}", today_sessions);

                println!("\n=== Usage ===");
                println!("  Sessions:      {}", stats.total_sessions);
                println!("  Input:         {} tokens", db::format_tokens(stats.total_tokens_input));
                println!("  Output:        {} tokens", db::format_tokens(stats.total_tokens_output));
                println!("  Reasoning:     {} tokens", db::format_tokens(stats.total_tokens_reasoning));
                println!("  Cache Read:    {} tokens", db::format_tokens(stats.total_tokens_cache_read));

                if !stats.by_model.is_empty() {
                    println!("\n=== Models ===");
                    for m in &stats.by_model {
                        println!("  {:<30} {:>6} sessions  in: {}  out: {}",
                            m.model_id, m.sessions,
                            db::format_tokens(m.tokens_input), db::format_tokens(m.tokens_output));
                    }
                }
                if !stats.by_project.is_empty() {
                    println!("\n=== Projects ===");
                    for p in &stats.by_project {
                        let name: String = p.project_name.chars().take(30).collect();
                        let name = if p.project_name.len() > 30 {
                            format!("{}…", name)
                        } else {
                            name
                        };
                        println!("  {:<30} {:>6} sessions",
                            name, p.sessions);
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
                render_mini(&conn)?;
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

fn render_mini(conn: &rusqlite::Connection) -> Result<(), Box<dyn std::error::Error>> {
    let stats = db::aggregate_stats(conn, None)?;
    let today_parts = db::todays_part_count(conn).unwrap_or(0);
    let last_day = stats.daily.first();
    let today_tokens = last_day.map(|d| d.tokens_input + d.tokens_output).unwrap_or(0);
    let cfg = config::Config::load();

    let part_limit = config::DAILY_PART_LIMIT;
    let token_limit = cfg.daily_limit_tokens;

    println!("{} sessions | {} tokens | today: {}/{} req, {}/{} tokens",
        stats.total_sessions,
        db::format_tokens(stats.total_tokens_input + stats.total_tokens_output),
        today_parts,
        part_limit,
        db::format_tokens(today_tokens),
        db::format_tokens(token_limit),
    );
    Ok(())
}

fn export_csv(path: &str, stats: &db::AggregateStats) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = csv::Writer::from_path(path)?;

    // Summary rows
    wtr.write_record(&["type", "key", "value"])?;
    wtr.write_record(&["summary", "sessions", &stats.total_sessions.to_string()])?;
    wtr.write_record(&["summary", "tokens_input", &stats.total_tokens_input.to_string()])?;
    wtr.write_record(&["summary", "tokens_output", &stats.total_tokens_output.to_string()])?;
    wtr.write_record(&["summary", "tokens_reasoning", &stats.total_tokens_reasoning.to_string()])?;
    wtr.write_record(&["summary", "tokens_cache_read", &stats.total_tokens_cache_read.to_string()])?;

    // Daily breakdown
    for d in &stats.daily {
        wtr.write_record(&["daily", &d.date, &d.sessions.to_string(), &d.tokens_input.to_string(), &d.tokens_output.to_string()])?;
    }

    // Model breakdown
    for m in &stats.by_model {
        wtr.write_record(&["model", &m.model_id, &m.sessions.to_string(), &m.tokens_input.to_string(), &m.tokens_output.to_string()])?;
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
                eprintln!("  Use 'xenxen --db-path <path>' to specify a valid database.");
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
