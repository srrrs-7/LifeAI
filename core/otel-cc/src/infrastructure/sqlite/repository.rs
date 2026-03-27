use anyhow::Result;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::domain::{
    model::{
        DailyStats, MetricsSummary, OverviewStats, ProjectStats, ProjectSummary, ScanState,
        Session, StatsResponse, TokenEvent, ToolCall,
    },
    port::{EventPort, OtlpPort, SessionPort, StatsPort},
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

        // プロジェクト別トークン集計
        let mut stmt = conn.prepare(
            "SELECT s.project,
                    COUNT(DISTINCT s.session_id),
                    COALESCE(SUM(t.input_tokens), 0),
                    COALESCE(SUM(t.output_tokens), 0),
                    COALESCE(SUM(t.cache_creation_tokens), 0),
                    COALESCE(SUM(t.cache_read_tokens), 0),
                    COALESCE(SUM(t.cost_usd), 0.0)
             FROM sessions s
             LEFT JOIN token_events t ON s.session_id = t.session_id
             GROUP BY s.project",
        )?;
        let mut projects: Vec<ProjectSummary> = stmt
            .query_map([], |r| {
                Ok(ProjectSummary {
                    project: r.get(0)?,
                    sessions: r.get(1)?,
                    input_tokens: r.get(2)?,
                    output_tokens: r.get(3)?,
                    cache_creation_tokens: r.get(4)?,
                    cache_read_tokens: r.get(5)?,
                    cost_usd: r.get(6)?,
                    tool_calls: 0,
                })
            })?
            .flatten()
            .collect();

        // プロジェクト別ツール数（Cartesian product 回避のため別クエリ）
        let mut stmt = conn.prepare(
            "SELECT s.project, COUNT(tc.id)
             FROM sessions s
             LEFT JOIN tool_calls tc ON s.session_id = tc.session_id
             GROUP BY s.project",
        )?;
        let tool_counts_by_project: std::collections::HashMap<String, i64> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .flatten()
            .collect();
        for p in &mut projects {
            p.tool_calls = *tool_counts_by_project.get(&p.project).unwrap_or(&0);
        }

        let mut stmt = conn.prepare(
            "SELECT entrypoint, COUNT(*) FROM sessions WHERE entrypoint IS NOT NULL GROUP BY entrypoint ORDER BY COUNT(*) DESC",
        )?;
        let entrypoint_counts = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
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
            entrypoint_counts,
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

impl StatsPort for SqliteRepository {
    fn query_stats(
        &self,
        period_days: Option<u32>,
        project: Option<&str>,
    ) -> Result<StatsResponse> {
        let conn = self.conn.lock().unwrap();
        let generated_at = chrono::Utc::now().to_rfc3339();

        // 期間フィルタ用カットオフ（None = 全期間）
        let cutoff = period_days
            .map(|d| (chrono::Utc::now() - chrono::Duration::days(d as i64)).to_rfc3339())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

        // ── セッション数 ─────────────────────────────────────────
        let (total_sessions, active_sessions): (i64, i64) = match project {
            Some(proj) => (
                conn.query_row(
                    "SELECT COUNT(*) FROM sessions WHERE project = ?1 AND last_seen_at >= ?2",
                    params![proj, cutoff],
                    |r| r.get(0),
                )
                .unwrap_or(0),
                conn.query_row(
                    "SELECT COUNT(*) FROM sessions WHERE project = ?1 AND is_active = 1 AND last_seen_at >= ?2",
                    params![proj, cutoff],
                    |r| r.get(0),
                )
                .unwrap_or(0),
            ),
            None => (
                conn.query_row(
                    "SELECT COUNT(*) FROM sessions WHERE last_seen_at >= ?1",
                    params![cutoff],
                    |r| r.get(0),
                )
                .unwrap_or(0),
                conn.query_row(
                    "SELECT COUNT(*) FROM sessions WHERE is_active = 1 AND last_seen_at >= ?1",
                    params![cutoff],
                    |r| r.get(0),
                )
                .unwrap_or(0),
            ),
        };

        // ── トークン集計 ─────────────────────────────────────────
        let (input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, cost_usd): (
            i64,
            i64,
            i64,
            i64,
            f64,
        ) = match project {
            Some(proj) => conn
                .query_row(
                    "SELECT COALESCE(SUM(te.input_tokens),0),
                            COALESCE(SUM(te.output_tokens),0),
                            COALESCE(SUM(te.cache_creation_tokens),0),
                            COALESCE(SUM(te.cache_read_tokens),0),
                            COALESCE(SUM(te.cost_usd),0.0)
                     FROM token_events te
                     JOIN sessions s ON te.session_id = s.session_id
                     WHERE s.project = ?1 AND te.timestamp >= ?2",
                    params![proj, cutoff],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .unwrap_or((0, 0, 0, 0, 0.0)),
            None => conn
                .query_row(
                    "SELECT COALESCE(SUM(input_tokens),0),
                            COALESCE(SUM(output_tokens),0),
                            COALESCE(SUM(cache_creation_tokens),0),
                            COALESCE(SUM(cache_read_tokens),0),
                            COALESCE(SUM(cost_usd),0.0)
                     FROM token_events WHERE timestamp >= ?1",
                    params![cutoff],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .unwrap_or((0, 0, 0, 0, 0.0)),
        };

        // ── ツールコール ─────────────────────────────────────────
        let (tool_calls, tool_errors): (i64, i64) = match project {
            Some(proj) => conn
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(tc.is_error),0)
                     FROM tool_calls tc
                     JOIN sessions s ON tc.session_id = s.session_id
                     WHERE s.project = ?1 AND tc.timestamp >= ?2",
                    params![proj, cutoff],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap_or((0, 0)),
            None => conn
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(is_error),0)
                     FROM tool_calls WHERE timestamp >= ?1",
                    params![cutoff],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap_or((0, 0)),
        };

        let total_with_cache = input_tokens + cache_read_tokens;
        let cache_hit_ratio = if total_with_cache > 0 {
            cache_read_tokens as f64 / total_with_cache as f64
        } else {
            0.0
        };

        let overview = OverviewStats {
            total_sessions,
            active_sessions,
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
            cost_usd,
            tool_calls,
            tool_errors,
            cache_hit_ratio,
        };

        // ── プロジェクト別内訳 ────────────────────────────────────
        let projects: Vec<ProjectStats> = match project {
            Some(proj) => {
                // 単一プロジェクト指定時は overview の値をそのまま使う
                vec![ProjectStats {
                    project: proj.to_string(),
                    sessions: total_sessions,
                    input_tokens,
                    output_tokens,
                    cache_creation_tokens,
                    cache_read_tokens,
                    cost_usd,
                    tool_calls,
                }]
            }
            None => {
                // トークン集計
                let mut stmt = conn.prepare(
                    "SELECT s.project,
                            COUNT(DISTINCT s.session_id),
                            COALESCE(SUM(te.input_tokens),0),
                            COALESCE(SUM(te.output_tokens),0),
                            COALESCE(SUM(te.cache_creation_tokens),0),
                            COALESCE(SUM(te.cache_read_tokens),0),
                            COALESCE(SUM(te.cost_usd),0.0)
                     FROM sessions s
                     LEFT JOIN token_events te ON s.session_id = te.session_id
                       AND te.timestamp >= ?1
                     WHERE s.last_seen_at >= ?1
                     GROUP BY s.project
                     ORDER BY SUM(te.cost_usd) DESC NULLS LAST",
                )?;
                let mut rows: Vec<ProjectStats> = stmt
                    .query_map(params![cutoff], |r| {
                        Ok(ProjectStats {
                            project: r.get(0)?,
                            sessions: r.get(1)?,
                            input_tokens: r.get(2)?,
                            output_tokens: r.get(3)?,
                            cache_creation_tokens: r.get(4)?,
                            cache_read_tokens: r.get(5)?,
                            cost_usd: r.get(6)?,
                            tool_calls: 0,
                        })
                    })?
                    .flatten()
                    .collect();

                // ツール数を別クエリで補完
                let mut stmt = conn.prepare(
                    "SELECT s.project, COUNT(tc.id)
                     FROM sessions s
                     LEFT JOIN tool_calls tc ON s.session_id = tc.session_id
                       AND tc.timestamp >= ?1
                     WHERE s.last_seen_at >= ?1
                     GROUP BY s.project",
                )?;
                let tc_map: std::collections::HashMap<String, i64> = stmt
                    .query_map(params![cutoff], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                    })?
                    .flatten()
                    .collect();
                for p in &mut rows {
                    p.tool_calls = *tc_map.get(&p.project).unwrap_or(&0);
                }
                rows
            }
        };

        // ── 日別内訳 ─────────────────────────────────────────────
        let daily: Vec<DailyStats> = match project {
            Some(proj) => {
                let mut stmt = conn.prepare(
                    "SELECT DATE(te.timestamp),
                            COALESCE(SUM(te.input_tokens),0),
                            COALESCE(SUM(te.output_tokens),0),
                            COALESCE(SUM(te.cache_creation_tokens),0),
                            COALESCE(SUM(te.cache_read_tokens),0),
                            COALESCE(SUM(te.cost_usd),0.0)
                     FROM token_events te
                     JOIN sessions s ON te.session_id = s.session_id
                     WHERE s.project = ?1 AND te.timestamp >= ?2
                     GROUP BY DATE(te.timestamp)
                     ORDER BY DATE(te.timestamp)",
                )?;
                let rows: Vec<DailyStats> = stmt
                    .query_map(params![proj, cutoff], |r| {
                        Ok(DailyStats {
                            date: r.get(0)?,
                            input_tokens: r.get(1)?,
                            output_tokens: r.get(2)?,
                            cache_creation_tokens: r.get(3)?,
                            cache_read_tokens: r.get(4)?,
                            cost_usd: r.get(5)?,
                        })
                    })?
                    .flatten()
                    .collect();
                rows
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT DATE(timestamp),
                            COALESCE(SUM(input_tokens),0),
                            COALESCE(SUM(output_tokens),0),
                            COALESCE(SUM(cache_creation_tokens),0),
                            COALESCE(SUM(cache_read_tokens),0),
                            COALESCE(SUM(cost_usd),0.0)
                     FROM token_events
                     WHERE timestamp >= ?1
                     GROUP BY DATE(timestamp)
                     ORDER BY DATE(timestamp)",
                )?;
                let rows: Vec<DailyStats> = stmt
                    .query_map(params![cutoff], |r| {
                        Ok(DailyStats {
                            date: r.get(0)?,
                            input_tokens: r.get(1)?,
                            output_tokens: r.get(2)?,
                            cache_creation_tokens: r.get(3)?,
                            cache_read_tokens: r.get(4)?,
                            cost_usd: r.get(5)?,
                        })
                    })?
                    .flatten()
                    .collect();
                rows
            }
        };

        Ok(StatsResponse {
            period_days,
            generated_at,
            overview,
            projects,
            daily,
        })
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

/// テスト用: SAVEPOINT を使ってテスト終了時に変更をロールバックする
///
/// `:memory:` DB はテストごとに独立しているが、トランザクション境界を明示することで
/// テストの意図を示し、将来の共有 DB への移行にも対応しやすくする。
#[cfg(test)]
impl SqliteRepository {
    pub fn with_rollback<F: FnOnce(&Self)>(&self, f: F) {
        {
            let conn = self.conn.lock().unwrap();
            conn.execute_batch("SAVEPOINT test_sp").unwrap();
        }
        f(self);
        {
            let conn = self.conn.lock().unwrap();
            conn.execute_batch("ROLLBACK TO SAVEPOINT test_sp; RELEASE test_sp")
                .unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{EventSource, Session, TokenEvent, ToolCall};
    use crate::domain::port::{EventPort, OtlpPort, SessionPort, StatsPort};
    use std::path::Path;

    fn repo() -> SqliteRepository {
        SqliteRepository::open(Path::new(":memory:")).unwrap()
    }

    /// `last_seen_at` を指定可能なセッション生成ヘルパー
    fn session_at(id: &str, project: &str, active: bool, last_seen_at: &str) -> Session {
        Session {
            session_id: id.to_string(),
            project: project.to_string(),
            cwd: None,
            git_branch: None,
            model: Some("claude-sonnet-4-6".to_string()),
            entrypoint: Some("cli".to_string()),
            version: None,
            started_at: last_seen_at.to_string(),
            last_seen_at: last_seen_at.to_string(),
            is_active: active,
        }
    }

    fn session(id: &str, project: &str, active: bool) -> Session {
        session_at(id, project, active, "2026-01-01T00:00:00Z")
    }

    /// `timestamp` を指定可能なトークンイベント生成ヘルパー
    fn token_ev_at(
        session_id: &str,
        input: i64,
        output: i64,
        cache_read: i64,
        cost: f64,
        timestamp: &str,
    ) -> TokenEvent {
        TokenEvent {
            session_id: session_id.to_string(),
            timestamp: timestamp.to_string(),
            model: Some("claude-sonnet-4-6".to_string()),
            input_tokens: input,
            output_tokens: output,
            cache_creation_tokens: 0,
            cache_read_tokens: cache_read,
            cost_usd: cost,
            source: EventSource::Log,
        }
    }

    fn token_ev(session_id: &str, input: i64, output: i64, cost: f64) -> TokenEvent {
        token_ev_at(session_id, input, output, 0, cost, "2026-01-01T00:00:00Z")
    }

    /// `timestamp` を指定可能なツールコール生成ヘルパー
    fn tool_call_at(session_id: &str, name: &str, is_error: bool, timestamp: &str) -> ToolCall {
        ToolCall {
            session_id: session_id.to_string(),
            tool_id: Some(format!("{session_id}-{name}")),
            timestamp: timestamp.to_string(),
            tool_name: name.to_string(),
            is_error,
            source: EventSource::Log,
        }
    }

    fn tool_call(session_id: &str, name: &str, is_error: bool) -> ToolCall {
        tool_call_at(session_id, name, is_error, "2026-01-01T00:00:00Z")
    }

    /// テスト用「現在時刻に近い」タイムスタンプ（期間フィルタで含まれる）
    fn recent() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    /// テスト用「遠い過去」タイムスタンプ（期間フィルタで除外される）
    fn old() -> &'static str {
        "2020-01-01T00:00:00Z"
    }

    // ── セッション ─────────────────────────────────────────────

    #[test]
    fn upsert_session_does_not_duplicate() {
        let r = repo();
        r.with_rollback(|r| {
            let mut s = session("s1", "proj", true);
            r.upsert_session(&s).unwrap();
            s.last_seen_at = "2026-02-01T00:00:00Z".to_string();
            r.upsert_session(&s).unwrap();

            let summary = r.load_summary().unwrap();
            assert_eq!(
                summary.total_sessions, 1,
                "upsert should not create duplicate rows"
            );
        });
    }

    #[test]
    fn active_sessions_counted_separately() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "p", true)).unwrap();
            r.upsert_session(&session("s2", "p", false)).unwrap();
            r.upsert_session(&session("s3", "p", true)).unwrap();

            let s = r.load_summary().unwrap();
            assert_eq!(s.total_sessions, 3);
            assert_eq!(s.active_sessions, 2);
        });
    }

    // ── トークン集計 ────────────────────────────────────────────

    #[test]
    fn token_events_aggregate_correctly() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "p", true)).unwrap();
            r.insert_token_event(&token_ev("s1", 100, 50, 0.001))
                .unwrap();
            r.insert_token_event(&token_ev("s1", 200, 80, 0.002))
                .unwrap();

            let s = r.load_summary().unwrap();
            assert_eq!(s.total_input_tokens, 300);
            assert_eq!(s.total_output_tokens, 130);
            assert!((s.total_cost_usd - 0.003).abs() < 1e-9);
        });
    }

    // ── ツールコール ────────────────────────────────────────────

    #[test]
    fn tool_calls_counted_with_error_split() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "p", true)).unwrap();
            r.insert_tool_call(&tool_call("s1", "Bash", false)).unwrap();
            r.insert_tool_call(&tool_call("s1", "Bash", true)).unwrap();
            r.insert_tool_call(&tool_call("s1", "Read", false)).unwrap();

            let s = r.load_summary().unwrap();
            assert_eq!(s.total_tool_calls, 3);
            assert_eq!(s.total_tool_errors, 1);

            let bash = s.tool_counts.iter().find(|(t, _, _)| t == "Bash").unwrap();
            assert_eq!((bash.1, bash.2), (2, 1));

            let read = s.tool_counts.iter().find(|(t, _, _)| t == "Read").unwrap();
            assert_eq!((read.1, read.2), (1, 0));
        });
    }

    // ── スキャン状態 ────────────────────────────────────────────

    #[test]
    fn scan_state_returns_none_before_set() {
        let r = repo();
        r.with_rollback(|r| {
            assert!(r.get_scan_state("/no/such/file.jsonl").unwrap().is_none());
        });
    }

    #[test]
    fn scan_state_roundtrip_and_overwrite() {
        let r = repo();
        r.with_rollback(|r| {
            let st = ScanState {
                last_modified: "111".to_string(),
                lines_processed: 10,
            };
            r.set_scan_state("/f.jsonl", &st).unwrap();

            let got = r.get_scan_state("/f.jsonl").unwrap().unwrap();
            assert_eq!(got.last_modified, "111");
            assert_eq!(got.lines_processed, 10);

            let st2 = ScanState {
                last_modified: "222".to_string(),
                lines_processed: 20,
            };
            r.set_scan_state("/f.jsonl", &st2).unwrap();
            let got2 = r.get_scan_state("/f.jsonl").unwrap().unwrap();
            assert_eq!(got2.lines_processed, 20);
        });
    }

    // ── 圧縮イベント ────────────────────────────────────────────

    #[test]
    fn compression_events_counted_in_summary() {
        let r = repo();
        r.with_rollback(|r| {
            r.insert_compression_event("s1", "2026-01-01T00:00:00Z", Some("compressed 5k tokens"))
                .unwrap();
            r.insert_compression_event("s1", "2026-01-02T00:00:00Z", None)
                .unwrap();

            let s = r.load_summary().unwrap();
            assert_eq!(s.total_compression_events, 2);
        });
    }

    // ── プロジェクト集計 ────────────────────────────────────────

    #[test]
    fn project_summary_groups_by_project() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "alpha", true)).unwrap();
            r.upsert_session(&session("s2", "beta", true)).unwrap();
            r.upsert_session(&session("s3", "alpha", true)).unwrap();
            r.insert_token_event(&token_ev("s1", 100, 50, 0.0)).unwrap();
            r.insert_token_event(&token_ev("s3", 200, 80, 0.0)).unwrap();

            let s = r.load_summary().unwrap();
            let alpha = s.projects.iter().find(|p| p.project == "alpha").unwrap();
            assert_eq!(alpha.sessions, 2);
            assert_eq!(alpha.input_tokens + alpha.output_tokens, 430); // 100+50+200+80

            let beta = s.projects.iter().find(|p| p.project == "beta").unwrap();
            assert_eq!(beta.sessions, 1);
            assert_eq!(beta.input_tokens + beta.output_tokens, 0);
        });
    }

    // ── プロジェクト集計 tool_calls ─────────────────────────────

    #[test]
    fn project_summary_includes_tool_calls_per_project() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "alpha", true)).unwrap();
            r.upsert_session(&session("s2", "beta", true)).unwrap();
            r.insert_tool_call(&tool_call("s1", "Bash", false)).unwrap();
            r.insert_tool_call(&tool_call("s1", "Read", false)).unwrap();
            r.insert_tool_call(&tool_call("s2", "Edit", false)).unwrap();

            let s = r.load_summary().unwrap();
            let alpha = s.projects.iter().find(|p| p.project == "alpha").unwrap();
            assert_eq!(alpha.tool_calls, 2, "alpha should have 2 tool calls");

            let beta = s.projects.iter().find(|p| p.project == "beta").unwrap();
            assert_eq!(beta.tool_calls, 1, "beta should have 1 tool call");
        });
    }

    // ── StatsPort::query_stats ───────────────────────────────────

    #[test]
    fn query_stats_no_filter_returns_all_data() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            r.insert_token_event(&token_ev("s1", 100, 50, 0.003))
                .unwrap();
            r.insert_tool_call(&tool_call("s1", "Bash", false)).unwrap();

            let stats = r.query_stats(None, None).unwrap();
            assert_eq!(stats.overview.total_sessions, 1);
            assert_eq!(stats.overview.input_tokens, 100);
            assert_eq!(stats.overview.output_tokens, 50);
            assert!((stats.overview.cost_usd - 0.003).abs() < 1e-9);
            assert_eq!(stats.overview.tool_calls, 1);
            assert_eq!(stats.period_days, None);
        });
    }

    #[test]
    fn query_stats_period_filter_excludes_old_events() {
        let r = repo();
        r.with_rollback(|r| {
            let now = recent();
            // 最近のセッション（期間内）
            r.upsert_session(&session_at("s1", "proj", true, &now))
                .unwrap();
            r.insert_token_event(&token_ev_at("s1", 200, 100, 0, 0.006, &now))
                .unwrap();
            // 古いセッション（期間外）
            r.upsert_session(&session_at("s2", "proj", false, old()))
                .unwrap();
            r.insert_token_event(&token_ev_at("s2", 999, 999, 0, 9.999, old()))
                .unwrap();

            let stats = r.query_stats(Some(7), None).unwrap();
            assert_eq!(
                stats.overview.total_sessions, 1,
                "old session should be excluded"
            );
            assert_eq!(
                stats.overview.input_tokens, 200,
                "old tokens should be excluded"
            );
            assert_eq!(stats.period_days, Some(7));
        });
    }

    #[test]
    fn query_stats_project_filter_scopes_to_project() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "alpha", true)).unwrap();
            r.upsert_session(&session("s2", "beta", true)).unwrap();
            r.insert_token_event(&token_ev("s1", 100, 50, 0.003))
                .unwrap();
            r.insert_token_event(&token_ev("s2", 999, 999, 9.999))
                .unwrap();

            let stats = r.query_stats(None, Some("alpha")).unwrap();
            assert_eq!(
                stats.overview.input_tokens, 100,
                "beta tokens must not appear"
            );
            assert_eq!(stats.projects.len(), 1);
            assert_eq!(stats.projects[0].project, "alpha");
        });
    }

    #[test]
    fn query_stats_overview_cache_hit_ratio() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            // input=100, cache_read=100 → ratio = 100/(100+100) = 0.5
            r.insert_token_event(&token_ev_at("s1", 100, 0, 100, 0.0, "2026-01-01T00:00:00Z"))
                .unwrap();

            let stats = r.query_stats(None, None).unwrap();
            assert!(
                (stats.overview.cache_hit_ratio - 0.5).abs() < 1e-9,
                "expected 0.5, got {}",
                stats.overview.cache_hit_ratio
            );
        });
    }

    #[test]
    fn query_stats_daily_breakdown_groups_by_date() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            r.insert_token_event(&token_ev_at("s1", 100, 0, 0, 0.003, "2026-03-25T10:00:00Z"))
                .unwrap();
            r.insert_token_event(&token_ev_at("s1", 200, 0, 0, 0.006, "2026-03-25T20:00:00Z"))
                .unwrap();
            r.insert_token_event(&token_ev_at("s1", 50, 0, 0, 0.0015, "2026-03-26T08:00:00Z"))
                .unwrap();

            let stats = r.query_stats(None, None).unwrap();
            let daily = &stats.daily;
            assert_eq!(daily.len(), 2, "should have 2 distinct dates");

            let day25 = daily.iter().find(|d| d.date == "2026-03-25").unwrap();
            assert_eq!(day25.input_tokens, 300, "day25: 100+200");

            let day26 = daily.iter().find(|d| d.date == "2026-03-26").unwrap();
            assert_eq!(day26.input_tokens, 50);
        });
    }

    #[test]
    fn query_stats_period_filter_excludes_old_tool_calls() {
        let r = repo();
        r.with_rollback(|r| {
            let now = recent();
            r.upsert_session(&session_at("s1", "proj", true, &now))
                .unwrap();
            r.insert_tool_call(&tool_call_at("s1", "Bash", false, &now))
                .unwrap();
            r.insert_tool_call(&tool_call_at("s1", "Read", true, old()))
                .unwrap(); // 古いエラー（除外されるべき）

            let stats = r.query_stats(Some(7), None).unwrap();
            assert_eq!(
                stats.overview.tool_calls, 1,
                "old tool call should be excluded"
            );
            assert_eq!(
                stats.overview.tool_errors, 0,
                "old error should be excluded"
            );
        });
    }

    // ── OTLP ポート ─────────────────────────────────────────────

    #[test]
    fn otlp_ports_insert_without_error() {
        let r = repo();
        r.with_rollback(|r| {
            r.insert_span(Some("t1"), Some("s1"), Some("my.span"), r#"{"raw":"data"}"#)
                .unwrap();
            r.insert_metric(Some("my.metric"), r#"{"v":1}"#).unwrap();
            r.insert_log(r#"{"body":"hello"}"#).unwrap();
            // NULL IDs も受け付ける
            r.insert_span(None, None, None, "{}").unwrap();
        });
    }

    // ── エントリーポイント集計 ────────────────────────────────────

    #[test]
    fn entrypoint_counts_groups_by_entrypoint() {
        let r = repo();
        r.with_rollback(|r| {
            // "cli" x2, "daily-report" x1
            r.upsert_session(&session("s1", "p", true)).unwrap(); // entrypoint = "cli"
            r.upsert_session(&session("s2", "p", true)).unwrap(); // entrypoint = "cli"
            r.upsert_session(&Session {
                entrypoint: Some("daily-report".to_string()),
                ..session("s3", "p", true)
            })
            .unwrap();

            let s = r.load_summary().unwrap();
            assert_eq!(s.entrypoint_counts.len(), 2);
            // DESC 順: cli(2) が先
            assert_eq!(s.entrypoint_counts[0], ("cli".to_string(), 2));
            assert_eq!(s.entrypoint_counts[1], ("daily-report".to_string(), 1));
        });
    }

    #[test]
    fn entrypoint_counts_excludes_null_entrypoints() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&Session {
                entrypoint: None,
                ..session("s1", "p", true)
            })
            .unwrap();
            r.upsert_session(&session("s2", "p", true)).unwrap(); // entrypoint = "cli"

            let s = r.load_summary().unwrap();
            assert_eq!(
                s.entrypoint_counts.len(),
                1,
                "NULL entrypoint must not appear"
            );
            assert_eq!(s.entrypoint_counts[0].0, "cli");
        });
    }

    #[test]
    fn entrypoint_counts_empty_when_no_sessions() {
        let r = repo();
        r.with_rollback(|r| {
            let s = r.load_summary().unwrap();
            assert!(s.entrypoint_counts.is_empty());
        });
    }
}
