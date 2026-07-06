# xenxen

A terminal dashboard for tracking your OpenCode Zen usage.

Reads your local OpenCode database and shows you how you're using AI — which models, which projects, how many tokens, and more.

## What xenxen does

- Shows how many sessions you've had
- Shows which AI models you use most
- Shows which projects use the most tokens
- Shows which tools you use most
- Tracks your daily usage over time
- Shows token usage (input, output, reasoning, cache)

## Features

- **Usage analytics** — see which models, projects, and tools you use most
- **Token tracking** — monitor input, output, reasoning, and cache tokens
- **Live dashboard** — colorful screen that updates automatically
- **Mini mode** — tiny one-line summary for status bars
- **CSV export** — save your stats to a file for Excel or analysis

## Installation

### From source

Requires Rust 1.85+ (edition 2024).

```bash
git clone <repo-url>
cd xenxen
cargo build --release
```

Binary will be at `target/release/xenxen`.

## How to Use xenxen (Easy Guide)

xenxen shows you how you're using OpenCode. It's like a report card for your AI usage.

### Step 1: Run xenxen

Just type this and press Enter:

```bash
xenxen
```

A colorful screen will appear showing your usage. It updates automatically every 5 seconds.

### Step 2: Look around

You'll see four tabs at the top:
- **Daily** — shows how many sessions you have each day
- **Model** — shows which AI models you use most
- **Project** — shows which projects use the most tokens
- **Tools** — shows which tools you use most

Use the arrow keys (↑ ↓) or the letters `j` and `k` to scroll up and down.

### Step 3: See your stats without the colorful screen

If you just want quick text information:

```bash
xenxen stats
```

This shows your stats in plain text that you can copy and paste.

### Step 4: Get a tiny status line

For a very small one-line summary (great for putting in your terminal prompt):

```bash
xenxen --mini
```

It shows something like: `61 sessions | 1.2M tokens | 5 today`

### Helpful Tips

- **Quit xenxen**: Press `q` or `Esc` to close the colorful screen
- **Help**: Press `?` to see all keyboard shortcuts
- **Refresh**: Press `r` to update your data right now

That's it! xenxen shows you how you're using OpenCode so you can understand your usage patterns.

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
# 61 sessions | 1.2M tokens | 45K today
```

### Export to CSV

```bash
xenxen --export-csv stats.csv
```

CSV format: `type,key,value` rows with summary, daily, model, and tool sections.

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
refresh_interval_secs = 5
```

### Fields

| Field | Default | Description |
|-------|---------|-------------|
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
