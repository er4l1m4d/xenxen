# AGENTS.md — AI Agent Conventions for xenxen

## Project Overview

xenxen is a Rust TUI dashboard that reads the local OpenCode SQLite database and displays usage analytics, balance tracking, and spending projections.

## Architecture

```
src/
  main.rs       — CLI entry point (clap), subcommands, config management
  db.rs         — SQLite queries, data structs, format helpers
  config.rs     — TOML config loading/saving, defaults
  balance.rs    — BalanceTracker, burn rate, projection math
  tui.rs        — ratatui TUI: header, overview, breakdown tabs, help overlay
```

## Module Responsibilities

### `db.rs`
- `find_database()` — probes 5+ locations for `opencode.db`
- `open_database()` — opens SQLite connection
- `get_all_sessions()`, `get_sessions_since()` — session queries
- `daily_breakdown()`, `model_breakdown()`, `project_breakdown()`, `tool_usage()` — aggregation queries
- `aggregate_stats()` — builds full `AggregateStats` struct
- `format_tokens()`, `format_cost()` — display helpers

### `config.rs`
- `Config` struct with serde defaults
- `Config::load()` — reads TOML, falls back to defaults on error
- `Config::save()` — writes TOML, creates parent dirs
- `Config::total_deposited()` — initial_balance + sum of topups

### `balance.rs`
- `BalanceTracker::new(config)` — takes a Config clone
- `BalanceTracker::snapshot(stats)` — computes `BalanceSnapshot`
- `BalanceStatus` enum: Healthy / Warning / Critical / Depleted
- `format_days_until_empty()`, `format_burn_rate()` — display helpers

### `tui.rs`
- `App` struct — holds conn, config, stats, UI state
- `run(terminal, app)` — event loop with tick-based refresh
- `render_header()` — balance bar + status
- `render_overview()` — left pane (balance/projections/usage)
- `render_breakdown()` — right pane (4 tabs with sortable tables)
- `render_help()` — keyboard shortcut overlay

## Data Flow

```
OpenCode DB (SQLite)
  → db::aggregate_stats() → AggregateStats
    → BalanceTracker::snapshot() → BalanceSnapshot
      → TUI renders header + overview + breakdown
```

## Key Patterns

### Error handling
- Functions return `Result<T, Box<dyn std::error::Error>>`
- Config loading falls back to defaults with `unwrap_or_default()` (now with warnings)
- DB errors propagate via `?` operator
- User-facing errors print to stderr with context

### Cloning
- `AggregateStats` derives `Clone` — used for sorting in-place in `render_breakdown`
- `BalanceTracker` takes `Config` by clone
- `App` holds `rusqlite::Connection` directly (not cloned)

### Sorting
- Tables sort in-place on `app.stats` fields before rendering
- Sort state: `sort_col: Option<u8>`, `sort_asc: bool`
- Resets when switching tabs

### Refresh cycle
1. `app.refresh()` queries DB, detects new activity
2. `terminal.draw()` renders UI
3. `event::poll(tick_rate)` waits for input or timeout
4. Loop repeats

## Conventions

- **No comments** in code unless explicitly requested
- **Minimal dependencies** — prefer stdlib over crates
- **Tests** in `#[cfg(test)] mod tests` at bottom of each module
- **Error messages** should tell users what went wrong and how to fix it
- **Config** uses serde defaults so missing fields are OK
- **TUI state** lives in `App` struct, not globals

## Testing

```bash
cargo test              # run all 16 tests
cargo test -- --nocapture  # show println! output
cargo build             # check compilation
cargo build --release   # optimize
```

## Build Notes

- Uses `x86_64-pc-windows-gnu` target (TDM-GCC)
- `rusqlite` bundled feature compiles SQLite from source
- `libgcc.a` must be copied to `libgcc_eh.a` for linker
- Release binary: ~6.6 MB

## Common Tasks

### Add a new TUI tab
1. Add tab constant (e.g., `5`) in `app.active_tab` handling
2. Add keybinding in event loop (`KeyCode::Char('5')`)
3. Add `build_*_table()` function
4. Add match arm in `render_breakdown()`
5. Update `max_scroll()` to include new tab's row count
6. Update help overlay

### Add a new CLI flag
1. Add field to `Cli` struct with `#[arg(...)]`
2. Handle in `match cli.command` or before it
3. Update README.md

### Add a new config field
1. Add field to `Config` struct with `#[serde(default = "...")]`
2. Add default function if non-trivial
3. Update `Config::default()` impl
4. Update `config show` output
5. Update README.md config section

## Known Limitations

- No public Zen balance API — balance tracking is local cost estimation
- Project names show as hash IDs when `project.name` is NULL in DB
- No file-watching (uses periodic polling for refresh)
- Single-user only (reads one OpenCode DB)
