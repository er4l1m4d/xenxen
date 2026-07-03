# xenxen

A terminal dashboard for tracking OpenCode Zen usage, balance, and spending projections.

Reads your local OpenCode SQLite database and displays real-time cost analytics in a TUI (terminal user interface).

## Features

- **Balance tracking** — remaining balance, burn rate, days until depleted
- **Session analytics** — daily, by model, by project, by tool usage
- **Live dashboard** — auto-refreshing TUI with keyboard navigation
- **Mini mode** — compact output for status bar embedding
- **CSV export** — dump stats for Excel/analysis
- **Auto-reload alerts** — warns when balance drops below threshold

## Installation

### From source

Requires Rust 1.70+.

```bash
git clone <repo-url>
cd xenxen
cargo build --release
```

Binary will be at `target/release/xenxen`.

### Pre-built binaries

Download from releases for your platform:
- `xenxen-x86_64-pc-windows-msvc.exe` — Windows
- `xenxen-x86_64-unknown-linux-gnu` — Linux
- `xenxen-x86_64-apple-darwin` — macOS

## Usage

### Interactive dashboard (default)

```bash
xenxen
```

### Plain text stats

```bash
xenxen stats
xenxen stats --days 7          # last 7 days only
xenxen stats --json            # JSON output
```

### Mini mode (status bars)

```bash
xenxen --mini
# Output:
# $38.30 OK [██████████████░] (96%)
# sessions: 61 | burn: $0.09/day | left: 1+ years
# last day: 5 sessions, $0.42
```

### Export to CSV

```bash
xenxen --export-csv stats.csv
```

CSV format: `type,key,value` rows with summary, daily, model, and tool sections.

### Configuration

```bash
xenxen config show                           # view config
xenxen config set-initial-balance 20.0       # set starting balance
xenxen config add-topup 20.0                 # record a top-up
xenxen config add-topup 20.0 --date 2026-06-15 --note "monthly"
```

## Keyboard shortcuts (TUI)

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit |
| `Ctrl+C` | Force quit |
| `?` | Toggle help |
| `1` `2` `3` `4` | Switch tabs (Daily / Model / Project / Tools) |
| `↑` `↓` / `j` `k` | Scroll up/down |
| `PgUp` `PgDn` | Page up/down |
| `Home` / `End` | Jump to top/bottom |
| `Tab` / `Shift+Tab` | Cycle sort column |
| `Space` | Toggle sort ascending/descending |
| `r` | Refresh data |

## Configuration file

Located at `~/.config/xenxen/config.toml`:

```toml
initial_balance = 20.0
auto_reload_threshold = 5.0
auto_reload_amount = 20.0
refresh_interval_secs = 5

[[topups]]
date = "2026-06-15"
amount = 20.0
note = "monthly top-up"
```

### Fields

| Field | Default | Description |
|-------|---------|-------------|
| `initial_balance` | `0.0` | Starting balance when you first set up tracking |
| `auto_reload_threshold` | `5.0` | Balance level that triggers a "Critical" warning |
| `auto_reload_amount` | `20.0` | Amount that would be auto-reloaded ( informational ) |
| `refresh_interval_secs` | `5` | Dashboard refresh interval in seconds |

## Database detection

xenxen auto-detects the OpenCode database by probing:

1. `$OPENCODE_DB` environment variable
2. `%LOCALAPPDATA%\opencode\opencode.db` (Windows)
3. `~/.local/share/opencode/opencode.db` (Linux/macOS)
4. `$XDG_DATA_HOME/opencode/opencode.db`
5. `%APPDATA%\opencode\opencode.db` (Windows fallback)

Override with `--db-path` or `$OPENCODE_DB`.

## Dependencies

- **ratatui** + **crossterm** — terminal UI
- **rusqlite** (bundled) — SQLite database access
- **clap** — CLI argument parsing
- **chrono** — date/time handling
- **serde** + **toml** + **serde_json** — config and serialization
- **csv** — CSV export
- **dirs** — platform config directories

## License

MIT
