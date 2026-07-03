use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::balance::{self, BalanceTracker, BalanceStatus};
use crate::config::Config;
use crate::db::{self, AggregateStats};

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct App {
    conn: rusqlite::Connection,
    config: Config,
    running: bool,
    active_tab: u8,
    show_help: bool,
    scroll_offset: u16,
    stats: AggregateStats,
    last_refresh: String,
    new_activity: bool,
    sort_col: Option<u8>,
    sort_asc: bool,
}

impl App {
    pub fn new(conn: rusqlite::Connection, config: Config) -> Self {
        Self {
            conn,
            config,
            running: true,
            active_tab: 1,
            show_help: false,
            scroll_offset: 0,
            stats: empty_stats(),
            last_refresh: String::new(),
            new_activity: false,
            sort_col: None,
            sort_asc: true,
        }
    }

    pub fn refresh(&mut self) {
        let prev = self.stats.total_sessions;
        self.stats = db::aggregate_stats(&self.conn, None).unwrap_or_else(|_| empty_stats());
        self.last_refresh = chrono::Local::now().format("%H:%M:%S").to_string();
        self.new_activity = self.stats.total_sessions > prev && prev > 0;
    }

    fn max_scroll(&self) -> u16 {
        let rows = match self.active_tab {
            1 => self.stats.daily.len(),
            2 => self.stats.by_model.len(),
            3 => self.stats.by_project.len(),
            4 => self.stats.top_tools.len(),
            _ => 0,
        };
        rows as u16
    }
}

fn empty_stats() -> AggregateStats {
    AggregateStats {
        total_sessions: 0,
        total_cost: 0.0,
        total_tokens_input: 0,
        total_tokens_output: 0,
        total_tokens_reasoning: 0,
        total_tokens_cache_read: 0,
        total_tokens_cache_write: 0,
        daily: vec![],
        by_model: vec![],
        by_project: vec![],
        top_tools: vec![],
    }
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

pub fn run(
    mut terminal: Terminal<impl Backend>,
    mut app: App,
) -> Result<(), Box<dyn std::error::Error>> {
    let tick_rate = Duration::from_secs(app.config.refresh_interval_secs);

    loop {
        app.refresh();
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                // Ctrl+C always quits
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    app.running = false;
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.running = false,
                    KeyCode::Char('?') => app.show_help = !app.show_help,
                    KeyCode::Char('r') => {}
                    KeyCode::Char('1') => { app.active_tab = 1; app.scroll_offset = 0; app.sort_col = None; }
                    KeyCode::Char('2') => { app.active_tab = 2; app.scroll_offset = 0; app.sort_col = None; }
                    KeyCode::Char('3') => { app.active_tab = 3; app.scroll_offset = 0; app.sort_col = None; }
                    KeyCode::Char('4') => { app.active_tab = 4; app.scroll_offset = 0; app.sort_col = None; }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.scroll_offset = app.scroll_offset.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.scroll_offset = (app.scroll_offset + 1).min(app.max_scroll());
                    }
                    KeyCode::PageUp => {
                        app.scroll_offset = app.scroll_offset.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        app.scroll_offset = (app.scroll_offset + 10).min(app.max_scroll());
                    }
                    KeyCode::Home => app.scroll_offset = 0,
                    KeyCode::End => app.scroll_offset = app.max_scroll(),
                    KeyCode::Tab => {
                        // Cycle sort column forward
                        let max_cols = match app.active_tab {
                            1 => 4, // daily: cost, sessions, in, out
                            2 => 5, // model: provider, cost, sessions, in, out
                            3 => 4, // project: cost, sessions, in, out
                            4 => 1, // tools: count only
                            _ => 0,
                        };
                        if max_cols > 0 {
                            app.sort_col = Some(match app.sort_col {
                                Some(c) if (c as u16) < (max_cols - 1) as u16 => c + 1,
                                Some(_) | None => 0,
                            });
                            app.sort_asc = true;
                        }
                    }
                    KeyCode::BackTab => {
                        // Cycle sort column backward
                        let max_cols = match app.active_tab {
                            1 => 4,
                            2 => 5,
                            3 => 4,
                            4 => 1,
                            _ => 0,
                        };
                        if max_cols > 0 {
                            app.sort_col = Some(match app.sort_col {
                                Some(0) | None => (max_cols - 1) as u8,
                                Some(c) => c - 1,
                            });
                            app.sort_asc = true;
                        }
                    }
                    KeyCode::Char(' ') => {
                        // Toggle sort direction
                        app.sort_asc = !app.sort_asc;
                    }
                    _ => {}
                }
            }
        }

        if !app.running {
            break;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// UI rendering
// ---------------------------------------------------------------------------

fn ui(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let tracker = BalanceTracker::new(app.config.clone());
    let snap = tracker.snapshot(&app.stats);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),   // Main
            Constraint::Length(1), // Footer
        ])
        .split(area);

    // ── Header ──────────────────────────────────────────────────────
    render_header(f, chunks[0], &snap, &app.last_refresh, app.new_activity);

    // ── Main: overview + breakdown ──────────────────────────────────
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(chunks[1]);

    render_overview(f, main_chunks[0], &snap, &app.stats);
    render_breakdown(f, main_chunks[1], app);

    // ── Footer ──────────────────────────────────────────────────────
    let sort_indicator = if let Some(col) = app.sort_col {
        let col_names = match app.active_tab {
            1 => ["", "cost", "sessions", "tokens_in", "tokens_out"],
            2 => ["model", "provider", "cost", "sessions", "tokens_in"],
            3 => ["project", "cost", "sessions", "tokens_in", "tokens_out"],
            4 => ["tool", "count", "", "", ""],
            _ => [""; 5],
        };
        let dir = if app.sort_asc { "↑" } else { "↓" };
        format!("  [Tab] Sort: {}{}", col_names[col as usize], dir)
    } else {
        "  [Tab] Sort".to_string()
    };
    let footer_text = if app.show_help {
        format!("  [q] Quit  [1-4] Tabs  [↑↓/jk] Scroll  [Tab] Sort  [?] Close Help")
    } else {
        format!("  [q] Quit  [1-4] Tabs  [↑↓] Scroll  [Tab] Sort  [?] Help{}", sort_indicator)
    };
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[2]);

    // ── Help overlay ────────────────────────────────────────────────
    if app.show_help {
        render_help(f, area);
    }
}

// ── Header ────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, area: Rect, snap: &balance::BalanceSnapshot, last_refresh: &str, new_activity: bool) {
    let status_icon = match snap.status {
        BalanceStatus::Healthy => "OK",
        BalanceStatus::Warning => "LOW",
        BalanceStatus::Critical => "!!",
        BalanceStatus::Depleted => "EMPTY",
    };
    let bg_color = match snap.status {
        BalanceStatus::Healthy => Color::Green,
        BalanceStatus::Warning => Color::Yellow,
        BalanceStatus::Critical => Color::Red,
        BalanceStatus::Depleted => Color::DarkGray,
    };

    // Build progress bar for balance
    let bar_width = 20;
    let fraction = if snap.total_deposited > 0.0 {
        (snap.remaining / snap.total_deposited).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (fraction * bar_width as f64) as usize;
    let empty = bar_width as usize - filled;
    let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

    let activity = if new_activity { "  ** NEW **" } else { "" };

    let header = Paragraph::new(format!(
        "  xenxen v0.1  │  {} {}  │  {} ({:.0}%)  │  {}{}",
        db::format_cost(snap.remaining),
        status_icon,
        bar,
        fraction * 100.0,
        last_refresh,
        activity,
    ))
    .style(Style::default().fg(Color::White).bg(bg_color))
    .alignment(Alignment::Center);
    f.render_widget(header, area);
}

// ── Overview pane ─────────────────────────────────────────────────────

fn render_overview(f: &mut Frame, area: Rect, snap: &balance::BalanceSnapshot, stats: &AggregateStats) {
    let burn_label = balance::format_burn_rate(snap.burn_rate_daily);
    let days_label = balance::format_days_until_empty(snap.days_until_empty);
    let auto_reload_line = if snap.will_auto_reload {
        format!("{} at < {}", db::format_cost(snap.auto_reload_amount), db::format_cost(snap.auto_reload_threshold))
    } else {
        "off".to_string()
    };

    let status_color = match snap.status {
        BalanceStatus::Healthy => Color::Green,
        BalanceStatus::Warning => Color::Yellow,
        BalanceStatus::Critical => Color::Red,
        BalanceStatus::Depleted => Color::DarkGray,
    };

    let lines = vec![
        // ── Balance section ──
        Line::from(Span::styled(
            "  BALANCE",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Remaining   ", Style::default()),
            Span::styled(
                db::format_cost(snap.remaining),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Deposited   ", Style::default()),
            Span::styled(db::format_cost(snap.total_deposited), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Spent       ", Style::default()),
            Span::styled(db::format_cost(snap.cumulative_spend), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Status      ", Style::default()),
            Span::styled(snap.status.label(), Style::default().fg(status_color)),
        ]),
        Line::from(""),
        // ── Projections section ──
        Line::from(Span::styled(
            "  PROJECTIONS",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Burn Rate   ", Style::default()),
            Span::styled(burn_label, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("  Days Left   ", Style::default()),
            Span::styled(days_label, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("  Auto-reload ", Style::default()),
            Span::styled(auto_reload_line, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        // ── Usage section ──
        Line::from(Span::styled(
            "  USAGE",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Sessions    ", Style::default()),
            Span::styled(format!("{}", stats.total_sessions), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Tokens In   ", Style::default()),
            Span::styled(db::format_tokens(stats.total_tokens_input), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Tokens Out  ", Style::default()),
            Span::styled(db::format_tokens(stats.total_tokens_output), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Reasoning   ", Style::default()),
            Span::styled(db::format_tokens(stats.total_tokens_reasoning), Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let overview = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Overview"));
    f.render_widget(overview, area);
}

// ── Breakdown pane ────────────────────────────────────────────────────

fn render_breakdown(f: &mut Frame, area: Rect, app: &mut App) {
    let tab_labels = ["1) Daily", "2) Model", "3) Project", "4) Tools"];
    let active_title = match app.active_tab {
        1 => "Daily",
        2 => "Model",
        3 => "Project",
        4 => "Tools",
        _ => "?",
    };

    // Sort data in-place before rendering
    if let Some(col) = app.sort_col {
        let asc = app.sort_asc;
        match app.active_tab {
            1 => {
                app.stats.daily.sort_by(|a, b| {
                    let ord = match col {
                        0 => a.date.cmp(&b.date),
                        1 => a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal),
                        2 => a.sessions.cmp(&b.sessions),
                        3 => a.tokens_input.cmp(&b.tokens_input),
                        4 => a.tokens_output.cmp(&b.tokens_output),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if asc { ord } else { ord.reverse() }
                });
            }
            2 => {
                app.stats.by_model.sort_by(|a, b| {
                    let ord = match col {
                        0 => a.model_id.cmp(&b.model_id),
                        1 => a.provider_id.cmp(&b.provider_id),
                        2 => a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal),
                        3 => a.sessions.cmp(&b.sessions),
                        4 => a.tokens_input.cmp(&b.tokens_input),
                        5 => a.tokens_output.cmp(&b.tokens_output),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if asc { ord } else { ord.reverse() }
                });
            }
            3 => {
                app.stats.by_project.sort_by(|a, b| {
                    let ord = match col {
                        0 => a.project_name.cmp(&b.project_name),
                        1 => a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal),
                        2 => a.sessions.cmp(&b.sessions),
                        3 => a.tokens_input.cmp(&b.tokens_input),
                        4 => a.tokens_output.cmp(&b.tokens_output),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if asc { ord } else { ord.reverse() }
                });
            }
            4 => {
                app.stats.top_tools.sort_by(|a, b| {
                    let ord = match col {
                        0 => a.tool_name.cmp(&b.tool_name),
                        1 => a.count.cmp(&b.count),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if asc { ord } else { ord.reverse() }
                });
            }
            _ => {}
        }
    }

    let help_line = Line::from(format!(
        "  [{}] [r] Refresh [j/k] Scroll [q] Quit",
        tab_labels.join("] [")
    ))
    .alignment(Alignment::Center);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Breakdown — {active_title}"))
        .title_bottom(help_line);

    let (header_cells, rows, widths) = match app.active_tab {
        1 => build_daily_table(app),
        2 => build_model_table(app),
        3 => build_project_table(app),
        4 => build_tools_table(app),
        _ => (vec![], vec![], vec![]),
    };

    let table = Table::new(rows, widths)
        .header(Row::new(header_cells).height(1))
        .block(block)
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(table, area);
}

fn build_daily_table(app: &App) -> (Vec<Cell<'_>>, Vec<Row<'_>>, Vec<Constraint>) {
    let headers = vec![
        Cell::from("Date").style(Style::default().fg(Color::Yellow)),
        Cell::from("Cost").style(Style::default().fg(Color::Yellow)),
        Cell::from("Sessions").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens In").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens Out").style(Style::default().fg(Color::Yellow)),
    ];
    let rows: Vec<Row> = app.stats.daily.iter().map(|d| {
        Row::new(vec![
            Cell::from(d.date.as_str()),
            Cell::from(db::format_cost(d.cost)),
            Cell::from(format!("{}", d.sessions)),
            Cell::from(db::format_tokens(d.tokens_input)),
            Cell::from(db::format_tokens(d.tokens_output)),
        ])
    }).collect();
    let widths = vec![
        Constraint::Percentage(25),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(22),
        Constraint::Percentage(23),
    ];
    (headers, rows, widths)
}

fn build_model_table(app: &App) -> (Vec<Cell<'_>>, Vec<Row<'_>>, Vec<Constraint>) {
    let headers = vec![
        Cell::from("Model").style(Style::default().fg(Color::Yellow)),
        Cell::from("Provider").style(Style::default().fg(Color::Yellow)),
        Cell::from("Cost").style(Style::default().fg(Color::Yellow)),
        Cell::from("Sessions").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens In").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens Out").style(Style::default().fg(Color::Yellow)),
    ];
    let rows: Vec<Row> = app.stats.by_model.iter().map(|m| {
        Row::new(vec![
            Cell::from(m.model_id.as_str()),
            Cell::from(m.provider_id.as_str()),
            Cell::from(db::format_cost(m.cost)),
            Cell::from(format!("{}", m.sessions)),
            Cell::from(db::format_tokens(m.tokens_input)),
            Cell::from(db::format_tokens(m.tokens_output)),
        ])
    }).collect();
    let widths = vec![
        Constraint::Percentage(22),
        Constraint::Percentage(16),
        Constraint::Percentage(12),
        Constraint::Percentage(14),
        Constraint::Percentage(18),
        Constraint::Percentage(18),
    ];
    (headers, rows, widths)
}

fn build_project_table(app: &App) -> (Vec<Cell<'_>>, Vec<Row<'_>>, Vec<Constraint>) {
    let headers = vec![
        Cell::from("Project").style(Style::default().fg(Color::Yellow)),
        Cell::from("Cost").style(Style::default().fg(Color::Yellow)),
        Cell::from("Sessions").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens In").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens Out").style(Style::default().fg(Color::Yellow)),
    ];
    let rows: Vec<Row> = app.stats.by_project.iter().map(|p| {
        let name = if p.project_name.len() > 30 {
            format!("{}…", &p.project_name[..29])
        } else {
            p.project_name.clone()
        };
        Row::new(vec![
            Cell::from(name),
            Cell::from(db::format_cost(p.cost)),
            Cell::from(format!("{}", p.sessions)),
            Cell::from(db::format_tokens(p.tokens_input)),
            Cell::from(db::format_tokens(p.tokens_output)),
        ])
    }).collect();
    let widths = vec![
        Constraint::Percentage(30),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ];
    (headers, rows, widths)
}

fn build_tools_table(app: &App) -> (Vec<Cell<'_>>, Vec<Row<'_>>, Vec<Constraint>) {
    let headers = vec![
        Cell::from("Tool").style(Style::default().fg(Color::Yellow)),
        Cell::from("Count").style(Style::default().fg(Color::Yellow)),
    ];
    let rows: Vec<Row> = app.stats.top_tools.iter().map(|t| {
        Row::new(vec![
            Cell::from(t.tool_name.as_str()),
            Cell::from(format!("{}", t.count)),
        ])
    }).collect();
    let widths = vec![Constraint::Percentage(60), Constraint::Percentage(40)];
    (headers, rows, widths)
}

// ── Help overlay ──────────────────────────────────────────────────────

fn render_help(f: &mut Frame, area: Rect) {
    let help_area = centered_rect(50, 60, area);

    let text = vec![
        Line::from(Span::styled(
            "  xenxen — Keyboard Shortcuts",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q / Esc      ", Style::default().fg(Color::Cyan)),
            Span::styled("Quit", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C       ", Style::default().fg(Color::Cyan)),
            Span::styled("Force quit", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  ?            ", Style::default().fg(Color::Cyan)),
            Span::styled("Toggle this help", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  1-4          ", Style::default().fg(Color::Cyan)),
            Span::styled("Switch tabs", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  r            ", Style::default().fg(Color::Cyan)),
            Span::styled("Refresh data", Style::default()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  ↑ / k        ", Style::default().fg(Color::Cyan)),
            Span::styled("Scroll up", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  ↓ / j        ", Style::default().fg(Color::Cyan)),
            Span::styled("Scroll down", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  PgUp / PgDn  ", Style::default().fg(Color::Cyan)),
            Span::styled("Page up/down", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Home / End   ", Style::default().fg(Color::Cyan)),
            Span::styled("Jump to top/bottom", Style::default()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Sorting",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Tab          ", Style::default().fg(Color::Cyan)),
            Span::styled("Next sort column", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Shift+Tab    ", Style::default().fg(Color::Cyan)),
            Span::styled("Prev sort column", Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Space        ", Style::default().fg(Color::Cyan)),
            Span::styled("Toggle asc/desc", Style::default()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press ? to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(Clear, help_area);
    f.render_widget(help, help_area);
}

/// Create a centered rect with given percentage of width and height.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
