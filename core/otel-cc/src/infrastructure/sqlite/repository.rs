use anyhow::Result;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::domain::{
    model::{MetricsSummary, ProjectSummary, ScanState, Session, TokenEvent, ToolCall},
    port::{EventPort, OtlpPort, SessionPort},
};

pub struct SqliteRepository {
    conn: Mutex<Connection>,
}

impl SqliteRepository {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let repo = Self {
            conn: Mutex::new(conn),
        };
        repo.init_schema()?;
        Ok(repo)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                session_id   TEXT PRIMARY KEY,
                project      TEXT NOT NULL,
                cwd          TEXT,
                git_branch   TEXT,
                model        TEXT,
                entrypoint   TEXT,
                version      TEXT,
                started_at   TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                is_active    INTEGER DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS token_events (
                id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id            TEXT NOT NULL,
                timestamp             TEXT NOT NULL,
                model                 TEXT,
                input_tokens          INTEGER DEFAULT 0,
                output_tokens         INTEGER DEFAULT 0,
                cache_creation_tokens INTEGER DEFAULT 0,
                cache_read_tokens     INTEGER DEFAULT 0,
                cost_usd              REAL    DEFAULT 0.0,
                source                TEXT    DEFAULT 'log',
                FOREIGN KEY (session_id) REFERENCES sessions(session_id)
            );

            CREATE TABLE IF NOT EXISTS tool_calls (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id  TEXT NOT NULL,
                tool_id     TEXT,
                timestamp   TEXT NOT NULL,
                tool_name   TEXT NOT NULL,
                is_error    INTEGER DEFAULT 0,
                source      TEXT DEFAULT 'log',
                FOREIGN KEY (session_id) REFERENCES sessions(session_id)
            );

            CREATE TABLE IF NOT EXISTS scan_state (
                path            TEXT PRIMARY KEY,
                last_modified   TEXT NOT NULL,
                lines_processed INTEGER NOT NULL DEFAULT 0,
                scanned_at      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS otlp_spans (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                received_at  TEXT NOT NULL,
                trace_id     TEXT,
                span_id      TEXT,
                name         TEXT,
                payload_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS otlp_metrics (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                received_at  TEXT NOT NULL,
                name         TEXT,
                payload_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS otlp_logs (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                received_at  TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );

            -- コンテキスト圧縮イベント（system レコードから検出）
            CREATE TABLE IF NOT EXISTS compression_events (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                timestamp  TEXT NOT NULL,
                summary    TEXT          -- 圧縮時のサマリー文字列（あれば）
            );

            CREATE INDEX IF NOT EXISTS idx_compression_session ON compression_events(session_id);

            CREATE INDEX IF NOT EXISTS idx_token_events_session ON token_events(session_id);
            CREATE INDEX IF NOT EXISTS idx_token_events_time    ON token_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_session   ON tool_calls(session_id);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_name      ON tool_calls(tool_name);
        ",
        )?;
        Ok(())
    }
}

impl SessionPort for SqliteRepository {
    fn upsert_session(&self, s: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions
                (session_id, project, cwd, git_branch, model, entrypoint, version, started_at, last_seen_at, is_active)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(session_id) DO UPDATE SET
                model        = excluded.model,
                last_seen_at = excluded.last_seen_at,
                is_active    = excluded.is_active",
            params![
                s.session_id, s.project, s.cwd, s.git_branch,
                s.model, s.entrypoint, s.version,
                s.started_at, s.last_seen_at, s.is_active as i32,
            ],
        )?;
        Ok(())
    }

    fn get_scan_state(&self, path: &str) -> Result<Option<ScanState>> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT last_modified, lines_processed FROM scan_state WHERE path = ?1",
                params![path],
                |row| {
                    Ok(ScanState {
                        last_modified: row.get(0)?,
                        lines_processed: row.get::<_, i64>(1)? as usize,
                    })
                },
            )
            .ok();
        Ok(result)
    }

    fn set_scan_state(&self, path: &str, state: &ScanState) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scan_state (path, last_modified, lines_processed, scanned_at)
             VALUES (?1,?2,?3,?4)
             ON CONFLICT(path) DO UPDATE SET
                last_modified   = excluded.last_modified,
                lines_processed = excluded.lines_processed,
                scanned_at      = excluded.scanned_at",
            params![path, state.last_modified, state.lines_processed as i64, now],
        )?;
        Ok(())
    }

    fn insert_compression_event(
        &self,
        session_id: &str,
        timestamp: &str,
        summary: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO compression_events (session_id, timestamp, summary) VALUES (?1,?2,?3)",
            params![session_id, timestamp, summary],
        )?;
        Ok(())
    }

    fn load_summary(&self) -> Result<MetricsSummary> {
        let conn = self.conn.lock().unwrap();

        let total_sessions = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap_or(0);

        let active_sessions = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE is_active = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let token_row: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cache_creation_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0)
                 FROM token_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap_or((0, 0, 0, 0));

        let total_cost_usd = conn
            .query_row(
                "SELECT COALESCE(SUM(cost_usd), 0.0) FROM token_events",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        let total_tool_calls = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap_or(0);

        let total_tool_errors = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE is_error = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let total_compression_events = conn
            .query_row("SELECT COUNT(*) FROM compression_events", [], |r| r.get(0))
            .unwrap_or(0);

        let mut stmt = conn.prepare(
            "SELECT tool_name, COUNT(*), SUM(is_error) FROM tool_calls GROUP BY tool_name",
        )?;
        let tool_counts = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .flatten()
            .collect();

        let mut stmt = conn.prepare(
            "SELECT s.project,
                    COUNT(DISTINCT s.session_id),
                    COALESCE(SUM(t.input_tokens + t.output_tokens), 0),
                    COALESCE(SUM(t.cost_usd), 0.0)
             FROM sessions s
             LEFT JOIN token_events t ON s.session_id = t.session_id
             GROUP BY s.project",
        )?;
        let projects = stmt
            .query_map([], |r| {
                Ok(ProjectSummary {
                    project: r.get(0)?,
                    sessions: r.get(1)?,
                    total_tokens: r.get(2)?,
                    cost_usd: r.get(3)?,
                })
            })?
            .flatten()
            .collect();

        Ok(MetricsSummary {
            total_sessions,
            active_sessions,
            total_input_tokens: token_row.0,
            total_output_tokens: token_row.1,
            total_cache_creation_tokens: token_row.2,
            total_cache_read_tokens: token_row.3,
            total_cost_usd,
            total_tool_calls,
            total_tool_errors,
            total_compression_events,
            tool_counts,
            projects,
        })
    }
}

impl EventPort for SqliteRepository {
    fn insert_token_event(&self, e: &TokenEvent) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO token_events
                (session_id, timestamp, model, input_tokens, output_tokens,
                 cache_creation_tokens, cache_read_tokens, cost_usd, source)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                e.session_id,
                e.timestamp,
                e.model,
                e.input_tokens,
                e.output_tokens,
                e.cache_creation_tokens,
                e.cache_read_tokens,
                e.cost_usd,
                e.source.to_string(),
            ],
        )?;
        Ok(())
    }

    fn insert_tool_call(&self, t: &ToolCall) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tool_calls
                (session_id, tool_id, timestamp, tool_name, is_error, source)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                t.session_id,
                t.tool_id,
                t.timestamp,
                t.tool_name,
                t.is_error as i32,
                t.source.to_string(),
            ],
        )?;
        Ok(())
    }
}

impl OtlpPort for SqliteRepository {
    fn insert_span(
        &self,
        trace_id: Option<&str>,
        span_id: Option<&str>,
        name: Option<&str>,
        payload_json: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO otlp_spans (received_at, trace_id, span_id, name, payload_json)
             VALUES (?1,?2,?3,?4,?5)",
            params![now, trace_id, span_id, name, payload_json],
        )?;
        Ok(())
    }

    fn insert_metric(&self, name: Option<&str>, payload_json: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO otlp_metrics (received_at, name, payload_json) VALUES (?1,?2,?3)",
            params![now, name, payload_json],
        )?;
        Ok(())
    }

    fn insert_log(&self, payload_json: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO otlp_logs (received_at, payload_json) VALUES (?1,?2)",
            params![now, payload_json],
        )?;
        Ok(())
    }
}
