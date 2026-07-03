use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Database path detection
// ---------------------------------------------------------------------------

/// Find the OpenCode SQLite database by probing known locations.
pub fn find_database() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("OPENCODE_DB") {
        let path = PathBuf::from(&env_path);
        if path.is_absolute() && path.exists() {
            return Some(path);
        }
        if let Some(data_dir) = dirs::data_local_dir() {
            let resolved = data_dir.join("opencode").join(&env_path);
            if resolved.exists() {
                return Some(resolved);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let path = PathBuf::from(local_app_data)
                .join("opencode")
                .join("opencode.db");
            if path.exists() {
                return Some(path);
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let path = home
            .join(".local")
            .join("share")
            .join("opencode")
            .join("opencode.db");
        if path.exists() {
            return Some(path);
        }
    }

    if let Some(data_dir) = dirs::data_local_dir() {
        let path = data_dir.join("opencode").join("opencode.db");
        if path.exists() {
            return Some(path);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let path = PathBuf::from(app_data)
                .join("opencode")
                .join("opencode.db");
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

pub fn open_database(path: &PathBuf) -> Result<Connection> {
    let conn = Connection::open(path).map_err(|e| {
        rusqlite::Error::InvalidParameterName(format!(
            "Failed to open database at '{}': {}",
            path.display(), e
        ))
    })?;
    Ok(conn)
}

// ---------------------------------------------------------------------------
// Data structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub agent: Option<String>,
    pub model: Option<SessionModel>,
    pub cost: f64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
    pub time_created: i64,
    pub time_updated: i64,
    pub time_archived: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionModel {
    pub id: String,
    pub provider_id: String,
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySummary {
    pub date: String,
    pub cost: f64,
    pub sessions: usize,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSummary {
    pub model_id: String,
    pub provider_id: String,
    pub cost: f64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub sessions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsage {
    pub tool_name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub project_name: String,
    pub cost: f64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub sessions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStats {
    pub total_sessions: usize,
    pub total_cost: f64,
    pub total_tokens_input: i64,
    pub total_tokens_output: i64,
    pub total_tokens_reasoning: i64,
    pub total_tokens_cache_read: i64,
    pub total_tokens_cache_write: i64,
    pub daily: Vec<DailySummary>,
    pub by_model: Vec<ModelSummary>,
    pub by_project: Vec<ProjectSummary>,
    pub top_tools: Vec<ToolUsage>,
}

// ---------------------------------------------------------------------------
// Session queries
// ---------------------------------------------------------------------------

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<Session> {
    let model_json: Option<String> = row.get("model")?;
    let model = model_json.and_then(|s| {
        serde_json::from_str::<SessionModel>(&s).ok()
    });

    Ok(Session {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        title: row.get("title")?,
        agent: row.get("agent")?,
        model,
        cost: row.get("cost")?,
        tokens_input: row.get("tokens_input")?,
        tokens_output: row.get("tokens_output")?,
        tokens_reasoning: row.get("tokens_reasoning")?,
        tokens_cache_read: row.get("tokens_cache_read")?,
        tokens_cache_write: row.get("tokens_cache_write")?,
        time_created: row.get("time_created")?,
        time_updated: row.get("time_updated")?,
        time_archived: row.get("time_archived")?,
    })
}

const SESSION_COLS: &str = "\
    s.id, s.project_id, s.title, s.agent, s.model, s.cost, \
    s.tokens_input, s.tokens_output, s.tokens_reasoning, \
    s.tokens_cache_read, s.tokens_cache_write, \
    s.time_created, s.time_updated, s.time_archived";

/// Get all non-archived sessions ordered by creation time.
pub fn get_all_sessions(conn: &Connection) -> Result<Vec<Session>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SESSION_COLS} FROM session s WHERE s.time_archived IS NULL ORDER BY s.time_created DESC"
    ))?;
    let rows = stmt.query_map([], |row| row_to_session(row))?;
    rows.collect()
}

/// Get sessions from the last N days.
pub fn get_sessions_since(conn: &Connection, days: u64) -> Result<Vec<Session>> {
    let cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(days as i64))
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0);

    let mut stmt = conn.prepare(&format!(
        "SELECT {SESSION_COLS} FROM session s \
         WHERE s.time_archived IS NULL AND s.time_created >= ?1 \
         ORDER BY s.time_created DESC"
    ))?;
    let rows = stmt.query_map([cutoff], |row| row_to_session(row))?;
    rows.collect()
}

/// Get sessions for a specific project.
pub fn get_sessions_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Session>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SESSION_COLS} FROM session s \
         WHERE s.time_archived IS NULL AND s.project_id = ?1 \
         ORDER BY s.time_created DESC"
    ))?;
    let rows = stmt.query_map([project_id], |row| row_to_session(row))?;
    rows.collect()
}

/// Get distinct project IDs with session counts.
pub fn get_projects(conn: &Connection) -> Result<Vec<(String, usize)>> {
    let mut stmt = conn.prepare(
        "SELECT project_id, COUNT(*) as cnt FROM session \
         WHERE time_archived IS NULL GROUP BY project_id ORDER BY cnt DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
    })?;
    rows.collect()
}

// ---------------------------------------------------------------------------
// Aggregation queries
// ---------------------------------------------------------------------------

/// Get daily cost/token breakdown.
pub fn daily_breakdown(conn: &Connection, days: Option<u64>) -> Result<Vec<DailySummary>> {
    let mut conditions = vec!["s.time_archived IS NULL".to_string()];
    if let Some(d) = days {
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(d as i64))
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);
        conditions.push(format!("s.time_created >= {cutoff}"));
    }
    let where_clause = conditions.join(" AND ");

    let mut stmt = conn.prepare(&format!(
        "SELECT \
            DATE(s.time_updated / 1000, 'unixepoch', 'localtime') as day, \
            COALESCE(SUM(s.cost), 0.0) as cost, \
            COUNT(*) as sessions, \
            COALESCE(SUM(s.tokens_input), 0) as tokens_in, \
            COALESCE(SUM(s.tokens_output), 0) as tokens_out, \
            COALESCE(SUM(s.tokens_reasoning), 0) as tokens_reasoning, \
            COALESCE(SUM(s.tokens_cache_read), 0) as cache_read, \
            COALESCE(SUM(s.tokens_cache_write), 0) as cache_write \
         FROM session s \
         WHERE {where_clause} \
         GROUP BY day \
         ORDER BY day DESC \
         LIMIT 60"
    ))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DailySummary {
                date: row.get(0)?,
                cost: row.get(1)?,
                sessions: row.get(2)?,
                tokens_input: row.get(3)?,
                tokens_output: row.get(4)?,
                tokens_reasoning: row.get(5)?,
                tokens_cache_read: row.get(6)?,
                tokens_cache_write: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Get per-model cost/token breakdown.
pub fn model_breakdown(conn: &Connection, days: Option<u64>) -> Result<Vec<ModelSummary>> {
    let mut conditions = vec![
        "s.time_archived IS NULL".to_string(),
        "s.model IS NOT NULL".to_string(),
    ];
    if let Some(d) = days {
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(d as i64))
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);
        conditions.push(format!("s.time_created >= {cutoff}"));
    }
    let where_clause = conditions.join(" AND ");

    let mut stmt = conn.prepare(&format!(
        "SELECT \
            COALESCE(json_extract(s.model, '$.id'), 'unknown') as model_id, \
            COALESCE(json_extract(s.model, '$.providerID'), 'unknown') as provider_id, \
            COALESCE(SUM(s.cost), 0.0) as cost, \
            COALESCE(SUM(s.tokens_input), 0) as tokens_in, \
            COALESCE(SUM(s.tokens_output), 0) as tokens_out, \
            COALESCE(SUM(s.tokens_reasoning), 0) as tokens_reasoning, \
            COUNT(*) as sessions \
         FROM session s \
         WHERE {where_clause} \
         GROUP BY model_id, provider_id \
         ORDER BY cost DESC"
    ))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ModelSummary {
                model_id: row.get(0)?,
                provider_id: row.get(1)?,
                cost: row.get(2)?,
                tokens_input: row.get(3)?,
                tokens_output: row.get(4)?,
                tokens_reasoning: row.get(5)?,
                sessions: row.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Get tool usage counts from the part table.
pub fn tool_usage(conn: &Connection, days: Option<u64>) -> Result<Vec<ToolUsage>> {
    let mut conditions = vec![
        "p.data LIKE '%\"type\":\"tool\"%'".to_string(),
    ];
    if let Some(d) = days {
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(d as i64))
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);
        conditions.push(format!("p.time_created >= {cutoff}"));
    }
    let where_clause = conditions.join(" AND ");

    let mut stmt = conn.prepare(&format!(
        "SELECT \
            json_extract(p.data, '$.tool') as tool_name, \
            COUNT(*) as cnt \
         FROM part p \
         WHERE {where_clause} \
         GROUP BY tool_name \
         ORDER BY cnt DESC \
         LIMIT 20"
    ))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ToolUsage {
                tool_name: row.get::<_, Option<String>>(0)?.unwrap_or_else(|| "unknown".to_string()),
                count: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Get per-project cost/token breakdown.
pub fn project_breakdown(conn: &Connection, days: Option<u64>) -> Result<Vec<ProjectSummary>> {
    let mut conditions = vec!["s.time_archived IS NULL".to_string()];
    if let Some(d) = days {
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(d as i64))
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);
        conditions.push(format!("s.time_created >= {cutoff}"));
    }
    let where_clause = conditions.join(" AND ");

    let mut stmt = conn.prepare(&format!(
        "SELECT \
            s.project_id, \
            COALESCE(p.name, s.project_id) as project_name, \
            COALESCE(SUM(s.cost), 0.0) as cost, \
            COALESCE(SUM(s.tokens_input), 0) as tokens_in, \
            COALESCE(SUM(s.tokens_output), 0) as tokens_out, \
            COUNT(*) as sessions \
         FROM session s \
         LEFT JOIN project p ON p.id = s.project_id \
         WHERE {where_clause} \
         GROUP BY s.project_id \
         ORDER BY cost DESC"
    ))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ProjectSummary {
                project_id: row.get(0)?,
                project_name: row.get(1)?,
                cost: row.get(2)?,
                tokens_input: row.get(3)?,
                tokens_output: row.get(4)?,
                sessions: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Build the full aggregate stats.
pub fn aggregate_stats(conn: &Connection, days: Option<u64>) -> Result<AggregateStats> {
    let sessions = match days {
        Some(d) => get_sessions_since(conn, d)?,
        None => get_all_sessions(conn)?,
    };

    let total_sessions = sessions.len();
    let total_cost = sessions.iter().map(|s| s.cost).sum();
    let total_tokens_input = sessions.iter().map(|s| s.tokens_input).sum();
    let total_tokens_output = sessions.iter().map(|s| s.tokens_output).sum();
    let total_tokens_reasoning = sessions.iter().map(|s| s.tokens_reasoning).sum();
    let total_tokens_cache_read = sessions.iter().map(|s| s.tokens_cache_read).sum();
    let total_tokens_cache_write = sessions.iter().map(|s| s.tokens_cache_write).sum();

    let daily = daily_breakdown(conn, days)?;
    let by_model = model_breakdown(conn, days)?;
    let by_project = project_breakdown(conn, days)?;
    let top_tools = tool_usage(conn, days)?;

    Ok(AggregateStats {
        total_sessions,
        total_cost,
        total_tokens_input,
        total_tokens_output,
        total_tokens_reasoning,
        total_tokens_cache_read,
        total_tokens_cache_write,
        daily,
        by_model,
        by_project,
        top_tools,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn format_tokens(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

pub fn format_cost(c: f64) -> String {
    format!("${:.2}", c)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE session (
                id TEXT PRIMARY KEY, project_id TEXT, parent_id TEXT, slug TEXT,
                directory TEXT, title TEXT, version TEXT, share_url TEXT,
                summary_additions INTEGER, summary_deletions INTEGER,
                summary_files INTEGER, summary_diffs TEXT, revert TEXT,
                permission TEXT, time_created INTEGER, time_updated INTEGER,
                time_compacting INTEGER, time_archived INTEGER,
                workspace_id TEXT, path TEXT, agent TEXT, model TEXT,
                cost REAL, tokens_input INTEGER, tokens_output INTEGER,
                tokens_reasoning INTEGER, tokens_cache_read INTEGER,
                tokens_cache_write INTEGER, metadata TEXT
            );
            CREATE TABLE part (
                id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT,
                time_created INTEGER, time_updated INTEGER, data TEXT
            );",
        )
        .unwrap();
        conn
    }

    fn insert_session(conn: &Connection, id: &str, cost: f64, tokens_in: i64, model: &str, ts: i64) {
        conn.execute(
            "INSERT INTO session (id, project_id, slug, directory, title, version, \
             time_created, time_updated, cost, tokens_input, tokens_output, \
             tokens_reasoning, tokens_cache_read, tokens_cache_write, model) \
             VALUES (?1, 'proj1', 'slug', '/tmp', 'Test', '1.0', ?2, ?2, ?3, ?4, 0, 0, 0, 0, ?5)",
            rusqlite::params![id, ts, cost, tokens_in, model],
        )
        .unwrap();
    }

    #[test]
    fn test_session_count_and_total_cost() {
        let conn = test_db();
        insert_session(&conn, "s1", 1.5, 1000, r#"{"id":"gpt-5","providerID":"openai"}"#, 1700000000000);
        insert_session(&conn, "s2", 0.5, 500, r#"{"id":"claude-sonnet-4-5","providerID":"anthropic"}"#, 1700010000000);

        let count: usize = conn.query_row("SELECT COUNT(*) FROM session", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 2);

        let total: f64 = conn.query_row("SELECT SUM(cost) FROM session", [], |r| r.get(0)).unwrap();
        assert!((total - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_daily_breakdown() {
        let conn = test_db();
        // Same day sessions
        insert_session(&conn, "s1", 1.0, 100, r#"{"id":"m1","providerID":"p1"}"#, 1700000000000);
        insert_session(&conn, "s2", 2.0, 200, r#"{"id":"m1","providerID":"p1"}"#, 1700000100000);

        let daily = daily_breakdown(&conn, None).unwrap();
        assert!(!daily.is_empty());
        assert!((daily[0].cost - 3.0).abs() < 0.001);
        assert_eq!(daily[0].sessions, 2);
    }

    #[test]
    fn test_model_breakdown() {
        let conn = test_db();
        insert_session(&conn, "s1", 1.0, 100, r#"{"id":"gpt-5","providerID":"openai"}"#, 1700000000000);
        insert_session(&conn, "s2", 2.0, 200, r#"{"id":"gpt-5","providerID":"openai"}"#, 1700000000000);
        insert_session(&conn, "s3", 0.5, 50, r#"{"id":"claude-sonnet-4-5","providerID":"anthropic"}"#, 1700000000000);

        let models = model_breakdown(&conn, None).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].model_id, "gpt-5");
        assert_eq!(models[0].sessions, 2);
    }

    #[test]
    fn test_tool_usage() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) \
             VALUES ('p1', 'm1', 's1', 1700000000000, 1700000000000, ?1)",
            rusqlite::params![r#"{"type":"tool","tool":"read","callID":"c1","state":{}}"#],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) \
             VALUES ('p2', 'm1', 's1', 1700000000000, 1700000000000, ?1)",
            rusqlite::params![r#"{"type":"tool","tool":"read","callID":"c2","state":{}}"#],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) \
             VALUES ('p3', 'm1', 's1', 1700000000000, 1700000000000, ?1)",
            rusqlite::params![r#"{"type":"tool","tool":"bash","callID":"c3","state":{}}"#],
        )
        .unwrap();

        let tools = tool_usage(&conn, None).unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].tool_name, "read");
        assert_eq!(tools[0].count, 2);
    }

    #[test]
    fn test_sessions_since() {
        let conn = test_db();
        let now = chrono::Utc::now().timestamp_millis();
        let day_ms = 86_400_000;
        insert_session(&conn, "s1", 1.0, 100, r#"{"id":"m1","providerID":"p1"}"#, now);
        insert_session(&conn, "s2", 1.0, 100, r#"{"id":"m1","providerID":"p1"}"#, now - 10 * day_ms);

        let recent = get_sessions_since(&conn, 5).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, "s1");

        let all = get_sessions_since(&conn, 365).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(2500000), "2.5M");
    }
}
