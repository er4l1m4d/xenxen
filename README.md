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

## How to Use xenxen (Easy Guide)

xenxen is like a personal money tracker for your OpenCode usage. It watches how much money you spend on AI models and tells you when you might run out.

### Step 1: Set up your starting balance

When you first use xenxen, tell it how much money you have. Open your terminal and type:

```bash
xenxen config set-initial-balance 20.0
```

This means you start with $20.00. You can change this number to whatever you want.

### Step 2: Run xenxen

Just type this and press Enter:

```bash
xenxen
```

A colorful screen will appear showing your money information. It updates automatically every 5 seconds.

### Step 3: Look around

You'll see four tabs at the top:
- **Daily** — shows how much you spend each day
- **Model** — shows which AI models cost you the most money
- **Project** — shows which projects use the most money
- **Tools** — shows which tools cost you money

Use the arrow keys (↑ ↓) or the letters `j` and `k` to scroll up and down.

### Step 4: Add more money

When you add money to your OpenCode account, tell xenxen:

```bash
xenxen config add-topup 20.0
```

This adds $20.00 to your tracked balance. You can add a note too:

```bash
xenxen config add-topup 20.0 --note "birthday money"
```

### Step 5: See your stats without the colorful screen

If you just want quick text information:

```bash
xenxen stats
```

This shows your stats in plain text that you can copy and paste.

### Step 6: Get a tiny status line

For a very small one-line summary (great for putting in your terminal prompt):

```bash
xenxen --mini
```

It shows something like: `$38.30 OK [██████████████░] (96%)`

### Helpful Tips

- **Quit xenxen**: Press `q` or `Esc` to close the colorful screen
- **Help**: Press `?` to see all keyboard shortcuts
- **Refresh**: Press `r` to update your data right now
- **Set low money warning**: xenxen warns you when your balance drops below $5.00. Change this with `xenxen config set-threshold 3.0`

### What the colors mean

- **Green** — You have plenty of money
- **Yellow** — Getting low, maybe add more soon
- **Red** — Very low! Add money now
- **Gray** — No more money left

That's it! xenxen watches your spending so you don't have to worry about running out of money unexpectedly.

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
