use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::config::{Config, DAILY_PART_LIMIT};
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
    let tick_rate = Duration::from_secs(5);

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
                        let max_cols = match app.active_tab {
                            1 => 4, // daily: date, sessions, in, out
                            2 => 4, // model: model, sessions, in, out
                            3 => 4, // project: project, sessions, in, out
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
                        let max_cols = match app.active_tab {
                            1 => 4,
                            2 => 4,
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

    // Compute today's usage
    let today_parts = db::todays_part_count(&app.conn).unwrap_or(0);
    let today_tokens = app.stats.daily.first()
        .map(|d| d.tokens_input + d.tokens_output)
        .unwrap_or(0);
    let today_sessions = app.stats.daily.first()
        .map(|d| d.sessions)
        .unwrap_or(0);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),   // Main
            Constraint::Length(1), // Footer
        ])
        .split(area);

    // ── Header ──────────────────────────────────────────────────────
    render_header(f, chunks[0], &app.stats, &app.last_refresh, app.new_activity, today_parts);

    // ── Main: overview + breakdown ──────────────────────────────────
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(chunks[1]);

    render_overview(f, main_chunks[0], &app.stats, today_parts, today_sessions, today_tokens, app.config.daily_limit_tokens);
    render_breakdown(f, main_chunks[1], app);

    // ── Footer ──────────────────────────────────────────────────────
    let sort_indicator = if let Some(col) = app.sort_col {
        let col_names = match app.active_tab {
            1 => ["", "sessions", "tokens_in", "tokens_out"],
            2 => ["model", "sessions", "tokens_in", "tokens_out"],
            3 => ["project", "sessions", "tokens_in", "tokens_out"],
            4 => ["tool", "count", "", ""],
            _ => [""; 4],
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

fn render_header(
    f: &mut Frame,
    area: Rect,
    stats: &AggregateStats,
    last_refresh: &str,
    new_activity: bool,
    today_parts: usize,
) {
    let total_tokens = stats.total_tokens_input + stats.total_tokens_output;
    let activity = if new_activity { "  ** NEW **" } else { "" };

    // Compute request bar (100/day limit)
    let part_limit = DAILY_PART_LIMIT;
    let part_fraction = (today_parts as f64 / part_limit as f64).clamp(0.0, 1.0);
    let part_color = if part_fraction > 0.9 {
        Color::Red
    } else if part_fraction > 0.7 {
        Color::Yellow
    } else {
        Color::Green
    };
    let bar_width = 8;
    let part_filled = (part_fraction * bar_width as f64) as usize;
    let part_empty = bar_width - part_filled;
    let part_bar = format!("[{}{}]", "█".repeat(part_filled), "░".repeat(part_empty));

    let header = Paragraph::new(format!(
        "  xenxen  │  {} sessions  │  {} tokens  │  Today: {}/{} {}  │  {}{}",
        stats.total_sessions,
        db::format_tokens(total_tokens),
        today_parts,
        part_limit,
        part_bar,
        last_refresh,
        activity,
    ))
    .style(Style::default().fg(Color::White).bg(part_color))
    .alignment(Alignment::Center);
    f.render_widget(header, area);
}

// ── Overview pane ─────────────────────────────────────────────────────

fn render_overview(
    f: &mut Frame,
    area: Rect,
    stats: &AggregateStats,
    today_parts: usize,
    today_sessions: usize,
    today_tokens: i64,
    daily_limit_tokens: i64,
) {
    let avg_sessions = if !stats.daily.is_empty() {
        stats.total_sessions as f64 / stats.daily.len() as f64
    } else {
        0.0
    };
    let avg_tokens = if !stats.daily.is_empty() {
        (stats.total_tokens_input + stats.total_tokens_output) as f64 / stats.daily.len() as f64
    } else {
        0.0
    };

    // Build request progress bar
    let part_limit = DAILY_PART_LIMIT;
    let part_fraction = (today_parts as f64 / part_limit as f64).clamp(0.0, 1.0);
    let part_color = if part_fraction > 0.9 {
        Color::Red
    } else if part_fraction > 0.7 {
        Color::Yellow
    } else {
        Color::Green
    };
    let bar_width = 12;
    let part_filled = (part_fraction * bar_width as f64) as usize;
    let part_empty = bar_width - part_filled;
    let part_bar = format!("[{}{}]", "█".repeat(part_filled), "░".repeat(part_empty));

    // Build token progress bar
    let token_fraction = if daily_limit_tokens > 0 {
        (today_tokens as f64 / daily_limit_tokens as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let token_color = if token_fraction > 0.9 {
        Color::Red
    } else if token_fraction > 0.7 {
        Color::Yellow
    } else {
        Color::Green
    };
    let token_filled = (token_fraction * bar_width as f64) as usize;
    let token_empty = bar_width - token_filled;
    let token_bar = format!("[{}{}]", "█".repeat(token_filled), "░".repeat(token_empty));

    let lines = vec![
        // ── Totals section ──
        Line::from(Span::styled(
            "  TOTALS",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Sessions     ", Style::default()),
            Span::styled(
                format!("{}", stats.total_sessions),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Tokens In    ", Style::default()),
            Span::styled(db::format_tokens(stats.total_tokens_input), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Tokens Out   ", Style::default()),
            Span::styled(db::format_tokens(stats.total_tokens_output), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        // ── Today's usage section ──
        Line::from(Span::styled(
            "  TODAY'S USAGE",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Sessions     ", Style::default()),
            Span::styled(format!("{}", today_sessions), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("  Requests     ", Style::default()),
            Span::styled(
                format!("{} / {} {}", today_parts, part_limit, part_bar),
                Style::default().fg(part_color),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Tokens       ", Style::default()),
            Span::styled(
                format!("{} / {} {}",
                    db::format_tokens(today_tokens),
                    db::format_tokens(daily_limit_tokens),
                    token_bar,
                ),
                Style::default().fg(token_color),
            ),
        ]),
        Line::from(""),
        // ── Averages section ──
        Line::from(Span::styled(
            "  AVERAGES / DAY",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Sessions     ", Style::default()),
            Span::styled(format!("{:.1}", avg_sessions), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Tokens       ", Style::default()),
            Span::styled(db::format_tokens(avg_tokens as i64), Style::default().fg(Color::DarkGray)),
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
                        1 => a.sessions.cmp(&b.sessions),
                        2 => a.tokens_input.cmp(&b.tokens_input),
                        3 => a.tokens_output.cmp(&b.tokens_output),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if asc { ord } else { ord.reverse() }
                });
            }
            2 => {
                app.stats.by_model.sort_by(|a, b| {
                    let ord = match col {
                        0 => a.model_id.cmp(&b.model_id),
                        1 => a.sessions.cmp(&b.sessions),
                        2 => a.tokens_input.cmp(&b.tokens_input),
                        3 => a.tokens_output.cmp(&b.tokens_output),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if asc { ord } else { ord.reverse() }
                });
            }
            3 => {
                app.stats.by_project.sort_by(|a, b| {
                    let ord = match col {
                        0 => a.project_name.cmp(&b.project_name),
                        1 => a.sessions.cmp(&b.sessions),
                        2 => a.tokens_input.cmp(&b.tokens_input),
                        3 => a.tokens_output.cmp(&b.tokens_output),
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
        Cell::from("Sessions").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens In").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens Out").style(Style::default().fg(Color::Yellow)),
    ];
    let rows: Vec<Row> = app.stats.daily.iter().map(|d| {
        Row::new(vec![
            Cell::from(d.date.as_str()),
            Cell::from(format!("{}", d.sessions)),
            Cell::from(db::format_tokens(d.tokens_input)),
            Cell::from(db::format_tokens(d.tokens_output)),
        ])
    }).collect();
    let widths = vec![
        Constraint::Percentage(30),
        Constraint::Percentage(18),
        Constraint::Percentage(26),
        Constraint::Percentage(26),
    ];
    (headers, rows, widths)
}

fn build_model_table(app: &App) -> (Vec<Cell<'_>>, Vec<Row<'_>>, Vec<Constraint>) {
    let headers = vec![
        Cell::from("Model").style(Style::default().fg(Color::Yellow)),
        Cell::from("Sessions").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens In").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens Out").style(Style::default().fg(Color::Yellow)),
    ];
    let rows: Vec<Row> = app.stats.by_model.iter().map(|m| {
        Row::new(vec![
            Cell::from(m.model_id.as_str()),
            Cell::from(format!("{}", m.sessions)),
            Cell::from(db::format_tokens(m.tokens_input)),
            Cell::from(db::format_tokens(m.tokens_output)),
        ])
    }).collect();
    let widths = vec![
        Constraint::Percentage(28),
        Constraint::Percentage(18),
        Constraint::Percentage(27),
        Constraint::Percentage(27),
    ];
    (headers, rows, widths)
}

fn build_project_table(app: &App) -> (Vec<Cell<'_>>, Vec<Row<'_>>, Vec<Constraint>) {
    let headers = vec![
        Cell::from("Project").style(Style::default().fg(Color::Yellow)),
        Cell::from("Sessions").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens In").style(Style::default().fg(Color::Yellow)),
        Cell::from("Tokens Out").style(Style::default().fg(Color::Yellow)),
    ];
    let rows: Vec<Row> = app.stats.by_project.iter().map(|p| {
        let name: String = p.project_name.chars().take(30).collect();
        let name = if p.project_name.len() > 30 {
            format!("{}…", name)
        } else {
            name
        };
        Row::new(vec![
            Cell::from(name),
            Cell::from(format!("{}", p.sessions)),
            Cell::from(db::format_tokens(p.tokens_input)),
            Cell::from(db::format_tokens(p.tokens_output)),
        ])
    }).collect();
    let widths = vec![
        Constraint::Percentage(30),
        Constraint::Percentage(18),
        Constraint::Percentage(26),
        Constraint::Percentage(26),
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
