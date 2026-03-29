use anyhow::Result;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::domain::{
    model::{
        DailyDataPoint, DailyStats, HourlyEfficiency, InsightState, MetricsSummary, ModelSummary,
        ModelSwitch, OverviewStats, ProjectStats, ProjectSummary, ScanState, Session,
        SessionCostProfile, StatsResponse, TokenEvent, ToolCall, ToolSequence, UserBenchmark,
        UserStats, UserSummary,
    },
    port::{
        AnalyticsPort, BenchmarkPort, EventPort, InsightStatePort, OptimizationPort, OtlpPort,
        SessionPort, StatsPort, TrendDataPort,
    },
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

            -- インサイトアノテーション送信状態（クールダウン管理）
            CREATE TABLE IF NOT EXISTS insight_states (
                key         TEXT PRIMARY KEY,
                last_sent_at TEXT NOT NULL,
                last_count  INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_token_events_session ON token_events(session_id);
            CREATE INDEX IF NOT EXISTS idx_token_events_time    ON token_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_session   ON tool_calls(session_id);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_name      ON tool_calls(tool_name);

            -- 複合インデックス（パフォーマンス改善 #3）
            CREATE INDEX IF NOT EXISTS idx_token_events_session_time
                ON token_events(session_id, timestamp);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_session_time
                ON tool_calls(session_id, timestamp);
        ",
        )?;

        // マイグレーション: sessions に user カラムを追加（既存 DB 対応）
        let has_user_col: bool = conn
            .prepare("PRAGMA table_info(sessions)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .flatten()
            .any(|name| name == "user");
        if !has_user_col {
            conn.execute_batch(
                "ALTER TABLE sessions ADD COLUMN user TEXT DEFAULT 'local';
                 CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user);",
            )?;
        }

        // user カラム追加後に複合インデックスを作成
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_sessions_user_project
                ON sessions(user, project);",
        )?;

        Ok(())
    }
}

impl SessionPort for SqliteRepository {
    fn upsert_session(&self, s: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions
                (session_id, project, user, cwd, git_branch, model, entrypoint, version, started_at, last_seen_at, is_active)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(session_id) DO UPDATE SET
                model        = excluded.model,
                last_seen_at = excluded.last_seen_at,
                is_active    = excluded.is_active",
            params![
                s.session_id, s.project, s.user, s.cwd, s.git_branch,
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

        // モデル別集計
        let mut stmt = conn.prepare(
            "SELECT COALESCE(model, 'unknown'),
                    COUNT(DISTINCT session_id),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cache_creation_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0),
                    COALESCE(SUM(cost_usd), 0.0)
             FROM token_events
             GROUP BY COALESCE(model, 'unknown')
             ORDER BY SUM(cost_usd) DESC",
        )?;
        let model_counts = stmt
            .query_map([], |r| {
                Ok(ModelSummary {
                    model: r.get(0)?,
                    sessions: r.get(1)?,
                    input_tokens: r.get(2)?,
                    output_tokens: r.get(3)?,
                    cache_creation_tokens: r.get(4)?,
                    cache_read_tokens: r.get(5)?,
                    cost_usd: r.get(6)?,
                })
            })?
            .flatten()
            .collect();

        // ユーザー別集計
        let mut stmt = conn.prepare(
            "SELECT COALESCE(s.user, 'local'),
                    COUNT(DISTINCT s.session_id),
                    COALESCE(SUM(t.input_tokens), 0),
                    COALESCE(SUM(t.output_tokens), 0),
                    COALESCE(SUM(t.cost_usd), 0.0)
             FROM sessions s
             LEFT JOIN token_events t ON s.session_id = t.session_id
             GROUP BY COALESCE(s.user, 'local')
             ORDER BY SUM(t.cost_usd) DESC NULLS LAST",
        )?;
        let mut user_counts: Vec<UserSummary> = stmt
            .query_map([], |r| {
                Ok(UserSummary {
                    user: r.get(0)?,
                    sessions: r.get(1)?,
                    input_tokens: r.get(2)?,
                    output_tokens: r.get(3)?,
                    cost_usd: r.get(4)?,
                    tool_calls: 0,
                })
            })?
            .flatten()
            .collect();

        // ユーザー別ツール数
        let mut stmt = conn.prepare(
            "SELECT COALESCE(s.user, 'local'), COUNT(tc.id)
             FROM sessions s
             LEFT JOIN tool_calls tc ON s.session_id = tc.session_id
             GROUP BY COALESCE(s.user, 'local')",
        )?;
        let tc_by_user: std::collections::HashMap<String, i64> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .flatten()
            .collect();
        for u in &mut user_counts {
            u.tool_calls = *tc_by_user.get(&u.user).unwrap_or(&0);
        }

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
            model_counts,
            user_counts,
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
        user: Option<&str>,
    ) -> Result<StatsResponse> {
        let conn = self.conn.lock().unwrap();
        let generated_at = chrono::Utc::now().to_rfc3339();

        // 期間フィルタ用カットオフ（None = 全期間）
        let cutoff = period_days
            .map(|d| (chrono::Utc::now() - chrono::Duration::days(d as i64)).to_rfc3339())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

        // ── 動的フィルタ構築 ──────────────────────────────────────
        let mut session_where = vec!["last_seen_at >= ?1".to_string()];
        let mut event_where = vec!["te.timestamp >= ?1".to_string()];
        let mut tc_where = vec!["tc.timestamp >= ?1".to_string()];
        let mut bind_values: Vec<String> = vec![cutoff.clone()];

        if let Some(proj) = project {
            let idx = bind_values.len() + 1;
            session_where.push(format!("project = ?{idx}"));
            event_where.push(format!("s.project = ?{idx}"));
            tc_where.push(format!("s.project = ?{idx}"));
            bind_values.push(proj.to_string());
        }
        if let Some(u) = user {
            let idx = bind_values.len() + 1;
            session_where.push(format!("user = ?{idx}"));
            event_where.push(format!("s.user = ?{idx}"));
            tc_where.push(format!("s.user = ?{idx}"));
            bind_values.push(u.to_string());
        }

        let session_filter = session_where.join(" AND ");
        let event_filter = event_where.join(" AND ");
        let tc_filter = tc_where.join(" AND ");

        // ── セッション数 ─────────────────────────────────────────
        let total_sessions: i64 = {
            let sql = format!("SELECT COUNT(*) FROM sessions WHERE {session_filter}");
            let mut stmt = conn.prepare(&sql)?;
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = bind_values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            stmt.query_row(params_ref.as_slice(), |r| r.get(0))
                .unwrap_or(0)
        };
        let active_sessions: i64 = {
            let sql =
                format!("SELECT COUNT(*) FROM sessions WHERE is_active = 1 AND {session_filter}");
            let mut stmt = conn.prepare(&sql)?;
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = bind_values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            stmt.query_row(params_ref.as_slice(), |r| r.get(0))
                .unwrap_or(0)
        };

        // ── トークン集計 ─────────────────────────────────────────
        let (input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, cost_usd) = {
            let sql = format!(
                "SELECT COALESCE(SUM(te.input_tokens),0),
                        COALESCE(SUM(te.output_tokens),0),
                        COALESCE(SUM(te.cache_creation_tokens),0),
                        COALESCE(SUM(te.cache_read_tokens),0),
                        COALESCE(SUM(te.cost_usd),0.0)
                 FROM token_events te
                 JOIN sessions s ON te.session_id = s.session_id
                 WHERE {event_filter}"
            );
            let mut stmt = conn.prepare(&sql)?;
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = bind_values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            stmt.query_row(params_ref.as_slice(), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, f64>(4)?,
                ))
            })
            .unwrap_or((0, 0, 0, 0, 0.0))
        };

        // ── ツールコール ─────────────────────────────────────────
        let (tool_calls, tool_errors) = {
            let sql = format!(
                "SELECT COUNT(*), COALESCE(SUM(tc.is_error),0)
                 FROM tool_calls tc
                 JOIN sessions s ON tc.session_id = s.session_id
                 WHERE {tc_filter}"
            );
            let mut stmt = conn.prepare(&sql)?;
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = bind_values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            stmt.query_row(params_ref.as_slice(), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            })
            .unwrap_or((0, 0))
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
        let projects: Vec<ProjectStats> = if let Some(proj) = project {
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
        } else {
            // ユーザーフィルタ用の条件を構築（project=None の else 分岐なので idx=2）
            let user_cond = if user.is_some() {
                " AND s.user = ?2".to_string()
            } else {
                String::new()
            };
            let sql = format!(
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
                 WHERE s.last_seen_at >= ?1{user_cond}
                 GROUP BY s.project
                 ORDER BY SUM(te.cost_usd) DESC NULLS LAST"
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut bind_proj: Vec<String> = vec![cutoff.clone()];
            if let Some(u) = user {
                bind_proj.push(u.to_string());
            }
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = bind_proj
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            let mut rows: Vec<ProjectStats> = stmt
                .query_map(params_ref.as_slice(), |r| {
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
            let sql2 = format!(
                "SELECT s.project, COUNT(tc.id)
                 FROM sessions s
                 LEFT JOIN tool_calls tc ON s.session_id = tc.session_id
                   AND tc.timestamp >= ?1
                 WHERE s.last_seen_at >= ?1{user_cond}
                 GROUP BY s.project"
            );
            let mut stmt2 = conn.prepare(&sql2)?;
            let tc_map: std::collections::HashMap<String, i64> = stmt2
                .query_map(params_ref.as_slice(), |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })?
                .flatten()
                .collect();
            for p in &mut rows {
                p.tool_calls = *tc_map.get(&p.project).unwrap_or(&0);
            }
            rows
        };

        // ── 日別内訳 ─────────────────────────────────────────────
        let daily: Vec<DailyStats> = {
            let sql = format!(
                "SELECT DATE(te.timestamp),
                        COALESCE(SUM(te.input_tokens),0),
                        COALESCE(SUM(te.output_tokens),0),
                        COALESCE(SUM(te.cache_creation_tokens),0),
                        COALESCE(SUM(te.cache_read_tokens),0),
                        COALESCE(SUM(te.cost_usd),0.0)
                 FROM token_events te
                 JOIN sessions s ON te.session_id = s.session_id
                 WHERE {event_filter}
                 GROUP BY DATE(te.timestamp)
                 ORDER BY DATE(te.timestamp)"
            );
            let mut stmt = conn.prepare(&sql)?;
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = bind_values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            let rows: Vec<DailyStats> = stmt
                .query_map(params_ref.as_slice(), |r| {
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
        };

        // ── ユーザー別内訳 ────────────────────────────────────────
        let users: Vec<UserStats> = {
            let sql = format!(
                "SELECT COALESCE(s.user, 'local'),
                        COUNT(DISTINCT s.session_id),
                        COALESCE(SUM(te.input_tokens),0),
                        COALESCE(SUM(te.output_tokens),0),
                        COALESCE(SUM(te.cache_creation_tokens),0),
                        COALESCE(SUM(te.cache_read_tokens),0),
                        COALESCE(SUM(te.cost_usd),0.0)
                 FROM sessions s
                 LEFT JOIN token_events te ON s.session_id = te.session_id
                   AND te.timestamp >= ?1
                 WHERE {session_filter}
                 GROUP BY COALESCE(s.user, 'local')
                 ORDER BY SUM(te.cost_usd) DESC NULLS LAST"
            );
            let mut stmt = conn.prepare(&sql)?;
            let params_ref: Vec<&dyn rusqlite::types::ToSql> = bind_values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            let mut rows: Vec<UserStats> = stmt
                .query_map(params_ref.as_slice(), |r| {
                    Ok(UserStats {
                        user: r.get(0)?,
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
            let sql2 = format!(
                "SELECT COALESCE(s.user, 'local'), COUNT(tc.id)
                 FROM sessions s
                 LEFT JOIN tool_calls tc ON s.session_id = tc.session_id
                   AND tc.timestamp >= ?1
                 WHERE {session_filter}
                 GROUP BY COALESCE(s.user, 'local')"
            );
            let mut stmt2 = conn.prepare(&sql2)?;
            let tc_map: std::collections::HashMap<String, i64> = stmt2
                .query_map(params_ref.as_slice(), |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })?
                .flatten()
                .collect();
            for u in &mut rows {
                u.tool_calls = *tc_map.get(&u.user).unwrap_or(&0);
            }
            rows
        };

        Ok(StatsResponse {
            period_days,
            generated_at,
            overview,
            projects,
            users,
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

impl InsightStatePort for SqliteRepository {
    fn get_insight_state(&self, key: &str) -> Result<Option<InsightState>> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT key, last_sent_at, last_count FROM insight_states WHERE key = ?1",
                params![key],
                |r| {
                    Ok(InsightState {
                        key: r.get(0)?,
                        last_sent_at: r.get(1)?,
                        last_count: r.get(2)?,
                    })
                },
            )
            .ok();
        Ok(result)
    }

    fn upsert_insight_state(&self, key: &str, sent_at: &str, count: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO insight_states (key, last_sent_at, last_count)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                last_sent_at = excluded.last_sent_at,
                last_count   = excluded.last_count",
            params![key, sent_at, count],
        )?;
        Ok(())
    }
}

impl TrendDataPort for SqliteRepository {
    fn daily_cost_per_session(
        &self,
        lookback_days: u32,
        user: Option<&str>,
    ) -> Result<Vec<DailyDataPoint>> {
        let conn = self.conn.lock().unwrap();
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(lookback_days as i64)).to_rfc3339();
        let user_join = if user.is_some() {
            " JOIN sessions s ON te.session_id = s.session_id"
        } else {
            ""
        };
        let user_cond = if user.is_some() {
            " AND s.user = ?2"
        } else {
            ""
        };
        let sql = format!(
            "SELECT DATE(te.timestamp) AS day,
                    COALESCE(SUM(te.cost_usd), 0.0) / MAX(1, COUNT(DISTINCT te.session_id))
             FROM token_events te{user_join}
             WHERE te.timestamp >= ?1{user_cond}
             GROUP BY DATE(te.timestamp)
             ORDER BY day"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<DailyDataPoint> = if let Some(u) = user {
            stmt.query_map(params![cutoff, u], |r| {
                Ok(DailyDataPoint {
                    date: r.get(0)?,
                    value: r.get(1)?,
                })
            })?
            .flatten()
            .collect()
        } else {
            stmt.query_map(params![cutoff], |r| {
                Ok(DailyDataPoint {
                    date: r.get(0)?,
                    value: r.get(1)?,
                })
            })?
            .flatten()
            .collect()
        };
        Ok(rows)
    }

    fn daily_cache_hit_ratio(
        &self,
        lookback_days: u32,
        user: Option<&str>,
    ) -> Result<Vec<DailyDataPoint>> {
        let conn = self.conn.lock().unwrap();
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(lookback_days as i64)).to_rfc3339();
        let user_join = if user.is_some() {
            " JOIN sessions s ON te.session_id = s.session_id"
        } else {
            ""
        };
        let user_cond = if user.is_some() {
            " AND s.user = ?2"
        } else {
            ""
        };
        let sql = format!(
            "SELECT DATE(te.timestamp) AS day,
                    CAST(COALESCE(SUM(te.cache_read_tokens), 0) AS REAL) /
                      MAX(1, COALESCE(SUM(te.input_tokens), 0) + COALESCE(SUM(te.cache_read_tokens), 0))
             FROM token_events te{user_join}
             WHERE te.timestamp >= ?1{user_cond}
             GROUP BY DATE(te.timestamp)
             ORDER BY day"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<DailyDataPoint> = if let Some(u) = user {
            stmt.query_map(params![cutoff, u], |r| {
                Ok(DailyDataPoint {
                    date: r.get(0)?,
                    value: r.get(1)?,
                })
            })?
            .flatten()
            .collect()
        } else {
            stmt.query_map(params![cutoff], |r| {
                Ok(DailyDataPoint {
                    date: r.get(0)?,
                    value: r.get(1)?,
                })
            })?
            .flatten()
            .collect()
        };
        Ok(rows)
    }

    fn daily_tool_error_rates(
        &self,
        lookback_days: u32,
        user: Option<&str>,
    ) -> Result<Vec<(String, Vec<DailyDataPoint>)>> {
        let conn = self.conn.lock().unwrap();
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(lookback_days as i64)).to_rfc3339();
        let user_join = if user.is_some() {
            " JOIN sessions s ON tc.session_id = s.session_id"
        } else {
            ""
        };
        let user_cond = if user.is_some() {
            " AND s.user = ?2"
        } else {
            ""
        };
        let sql = format!(
            "SELECT DATE(tc.timestamp) AS day,
                    tc.tool_name,
                    COUNT(*) AS total_calls,
                    SUM(tc.is_error) AS error_calls
             FROM tool_calls tc{user_join}
             WHERE tc.timestamp >= ?1{user_cond}
             GROUP BY DATE(tc.timestamp), tc.tool_name
             ORDER BY tc.tool_name, day"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<(String, String, i64, i64)> = if let Some(u) = user {
            stmt.query_map(params![cutoff, u], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .flatten()
            .collect()
        } else {
            stmt.query_map(params![cutoff], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .flatten()
            .collect()
        };

        // ツール名でグルーピング
        let mut map: std::collections::BTreeMap<String, Vec<DailyDataPoint>> =
            std::collections::BTreeMap::new();
        for (day, tool_name, total, errors) in rows {
            let rate = if total > 0 {
                errors as f64 / total as f64
            } else {
                0.0
            };
            map.entry(tool_name).or_default().push(DailyDataPoint {
                date: day,
                value: rate,
            });
        }
        Ok(map.into_iter().collect())
    }
}

// ── AnalyticsPort (#13) ─────────────────────────────────────────

impl AnalyticsPort for SqliteRepository {
    fn tool_usage_sequences(&self, limit: usize) -> Result<Vec<ToolSequence>> {
        let conn = self.conn.lock().unwrap();
        // LAG ウィンドウ関数で連続ツールペアを検出
        let sql = "
            WITH ordered AS (
                SELECT tool_name,
                       LAG(tool_name) OVER (PARTITION BY session_id ORDER BY id) AS prev_tool,
                       timestamp,
                       LAG(timestamp) OVER (PARTITION BY session_id ORDER BY id) AS prev_ts
                FROM tool_calls
            )
            SELECT prev_tool, tool_name,
                   COUNT(*) AS cnt,
                   AVG(
                       CASE WHEN prev_ts IS NOT NULL
                            THEN (julianday(timestamp) - julianday(prev_ts)) * 86400
                            ELSE 0 END
                   ) AS avg_interval
            FROM ordered
            WHERE prev_tool IS NOT NULL
            GROUP BY prev_tool, tool_name
            ORDER BY cnt DESC
            LIMIT ?1
        ";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                Ok(ToolSequence {
                    tool_a: r.get(0)?,
                    tool_b: r.get(1)?,
                    count: r.get(2)?,
                    avg_interval_secs: r.get(3)?,
                })
            })?
            .flatten()
            .collect();
        Ok(rows)
    }

    fn model_switching_patterns(&self) -> Result<Vec<ModelSwitch>> {
        let conn = self.conn.lock().unwrap();
        let sql = "
            WITH ordered AS (
                SELECT model,
                       LAG(model) OVER (PARTITION BY session_id ORDER BY id) AS prev_model
                FROM token_events
                WHERE model IS NOT NULL
            )
            SELECT prev_model, model, COUNT(*) AS cnt
            FROM ordered
            WHERE prev_model IS NOT NULL AND prev_model != model
            GROUP BY prev_model, model
            ORDER BY cnt DESC
        ";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(ModelSwitch {
                    from_model: r.get(0)?,
                    to_model: r.get(1)?,
                    count: r.get(2)?,
                })
            })?
            .flatten()
            .collect();
        Ok(rows)
    }

    fn hourly_efficiency(&self) -> Result<Vec<HourlyEfficiency>> {
        let conn = self.conn.lock().unwrap();
        let sql = "
            SELECT CAST(STRFTIME('%H', te.timestamp) AS INTEGER) AS hour,
                   COUNT(DISTINCT te.session_id) AS sessions,
                   COALESCE(SUM(te.cost_usd), 0.0) /
                       MAX(1, COUNT(DISTINCT te.session_id)) AS avg_cost,
                   CAST(COALESCE(SUM(te.input_tokens + te.output_tokens), 0) AS REAL) /
                       MAX(1, COUNT(DISTINCT te.session_id)) AS avg_tokens
            FROM token_events te
            GROUP BY CAST(STRFTIME('%H', te.timestamp) AS INTEGER)
            ORDER BY hour
        ";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(HourlyEfficiency {
                    hour: r.get::<_, i64>(0)? as u8,
                    sessions: r.get(1)?,
                    avg_cost_usd: r.get(2)?,
                    avg_tokens_per_session: r.get(3)?,
                })
            })?
            .flatten()
            .collect();
        Ok(rows)
    }
}

// ── OptimizationPort (#14) ──────────────────────────────────────

impl OptimizationPort for SqliteRepository {
    fn find_overprovisioned_sessions(
        &self,
        period_days: Option<u32>,
    ) -> Result<Vec<SessionCostProfile>> {
        let conn = self.conn.lock().unwrap();
        let cutoff = period_days
            .map(|d| (chrono::Utc::now() - chrono::Duration::days(d as i64)).to_rfc3339())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

        let sql = "
            SELECT s.session_id,
                   COALESCE(s.model, 'unknown'),
                   COALESCE(SUM(te.cost_usd), 0.0),
                   COALESCE(SUM(te.input_tokens), 0),
                   COALESCE(SUM(te.output_tokens), 0),
                   COALESCE(SUM(te.cache_creation_tokens), 0),
                   COALESCE(SUM(te.cache_read_tokens), 0),
                   (SELECT COUNT(*) FROM tool_calls tc WHERE tc.session_id = s.session_id) AS tool_count
            FROM sessions s
            LEFT JOIN token_events te ON s.session_id = te.session_id
            WHERE s.last_seen_at >= ?1
              AND LOWER(COALESCE(s.model, '')) LIKE '%opus%'
            GROUP BY s.session_id
        ";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(params![cutoff], |r| {
                Ok(SessionCostProfile {
                    session_id: r.get(0)?,
                    model: r.get(1)?,
                    cost_usd: r.get(2)?,
                    input_tokens: r.get(3)?,
                    output_tokens: r.get(4)?,
                    cache_creation_tokens: r.get(5)?,
                    cache_read_tokens: r.get(6)?,
                    tool_calls: r.get(7)?,
                })
            })?
            .flatten()
            .collect();
        Ok(rows)
    }
}

// ── BenchmarkPort (#15) ─────────────────────────────────────────

impl BenchmarkPort for SqliteRepository {
    fn user_efficiency_metrics(&self, period_days: Option<u32>) -> Result<Vec<UserBenchmark>> {
        let conn = self.conn.lock().unwrap();
        let cutoff = period_days
            .map(|d| (chrono::Utc::now() - chrono::Duration::days(d as i64)).to_rfc3339())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

        let sql = "
            SELECT COALESCE(s.user, 'local') AS u,
                   COUNT(DISTINCT s.session_id) AS sessions,
                   COALESCE(SUM(te.cost_usd), 0.0) AS total_cost,
                   COALESCE(SUM(te.input_tokens), 0) AS total_input,
                   COALESCE(SUM(te.cache_read_tokens), 0) AS total_cache_read,
                   (SELECT COUNT(*) FROM tool_calls tc
                    JOIN sessions s2 ON tc.session_id = s2.session_id
                    WHERE COALESCE(s2.user, 'local') = COALESCE(s.user, 'local')
                      AND tc.timestamp >= ?1) AS total_tools,
                   (SELECT COUNT(*) FROM tool_calls tc
                    JOIN sessions s2 ON tc.session_id = s2.session_id
                    WHERE COALESCE(s2.user, 'local') = COALESCE(s.user, 'local')
                      AND tc.is_error = 1
                      AND tc.timestamp >= ?1) AS total_errors
            FROM sessions s
            LEFT JOIN token_events te ON s.session_id = te.session_id
              AND te.timestamp >= ?1
            WHERE s.last_seen_at >= ?1
            GROUP BY COALESCE(s.user, 'local')
            ORDER BY total_cost DESC
        ";
        let mut stmt = conn.prepare(sql)?;
        let rows: Vec<UserBenchmark> = stmt
            .query_map(params![cutoff], |r| {
                let sessions: i64 = r.get(1)?;
                let total_cost: f64 = r.get(2)?;
                let total_input: i64 = r.get(3)?;
                let total_cache_read: i64 = r.get(4)?;
                let total_tools: i64 = r.get(5)?;
                let total_errors: i64 = r.get(6)?;

                let cost_per_session = if sessions > 0 {
                    total_cost / sessions as f64
                } else {
                    0.0
                };
                let total_with_cache = total_input + total_cache_read;
                let cache_hit_ratio = if total_with_cache > 0 {
                    total_cache_read as f64 / total_with_cache as f64
                } else {
                    0.0
                };
                let tool_error_rate = if total_tools > 0 {
                    total_errors as f64 / total_tools as f64
                } else {
                    0.0
                };

                Ok(UserBenchmark {
                    user: r.get(0)?,
                    sessions,
                    cost_per_session,
                    cache_hit_ratio,
                    tool_error_rate,
                    total_cost_usd: total_cost,
                    rank: 0, // ランクは application 層で計算
                })
            })?
            .flatten()
            .collect();
        Ok(rows)
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
    use crate::domain::port::{
        AnalyticsPort, BenchmarkPort, EventPort, OptimizationPort, OtlpPort, SessionPort, StatsPort,
    };
    use std::path::Path;

    fn repo() -> SqliteRepository {
        SqliteRepository::open(Path::new(":memory:")).unwrap()
    }

    /// `last_seen_at` を指定可能なセッション生成ヘルパー
    fn session_at(id: &str, project: &str, active: bool, last_seen_at: &str) -> Session {
        Session {
            session_id: id.to_string(),
            project: project.to_string(),
            user: "test-user".to_string(),
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

            let stats = r.query_stats(None, None, None).unwrap();
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

            let stats = r.query_stats(Some(7), None, None).unwrap();
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

            let stats = r.query_stats(None, Some("alpha"), None).unwrap();
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

            let stats = r.query_stats(None, None, None).unwrap();
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

            let stats = r.query_stats(None, None, None).unwrap();
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

            let stats = r.query_stats(Some(7), None, None).unwrap();
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

    // ── InsightStatePort ─────────────────────────────────────────

    #[test]
    fn insight_state_returns_none_before_set() {
        let r = repo();
        r.with_rollback(|r| {
            assert!(r.get_insight_state("unknown_key").unwrap().is_none());
        });
    }

    #[test]
    fn insight_state_roundtrip_stores_and_retrieves() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_insight_state("tool_error_rate:Grep", "2026-03-28T10:00:00Z", 5)
                .unwrap();
            let state = r
                .get_insight_state("tool_error_rate:Grep")
                .unwrap()
                .unwrap();
            assert_eq!(state.key, "tool_error_rate:Grep");
            assert_eq!(state.last_sent_at, "2026-03-28T10:00:00Z");
            assert_eq!(state.last_count, 5);
        });
    }

    #[test]
    fn insight_state_upsert_overwrites_existing() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_insight_state("compression_events", "2026-03-01T00:00:00Z", 3)
                .unwrap();
            r.upsert_insight_state("compression_events", "2026-03-28T00:00:00Z", 7)
                .unwrap();
            let state = r.get_insight_state("compression_events").unwrap().unwrap();
            assert_eq!(state.last_count, 7, "count should be updated to 7");
            assert_eq!(state.last_sent_at, "2026-03-28T00:00:00Z");
        });
    }

    #[test]
    fn insight_state_multiple_keys_independent() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_insight_state("key_a", "2026-03-28T10:00:00Z", 1)
                .unwrap();
            r.upsert_insight_state("key_b", "2026-03-28T11:00:00Z", 2)
                .unwrap();
            let a = r.get_insight_state("key_a").unwrap().unwrap();
            let b = r.get_insight_state("key_b").unwrap().unwrap();
            assert_eq!(a.last_count, 1);
            assert_eq!(b.last_count, 2);
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

    // ── TrendDataPort テスト ─────────────────────────────────────

    #[test]
    fn daily_cost_per_session_groups_by_date() {
        let r = repo();
        r.with_rollback(|r| {
            // 2セッション、3日分のイベント
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            r.upsert_session(&session("s2", "proj", true)).unwrap();
            r.insert_token_event(&token_ev_at("s1", 100, 50, 0, 5.0, "2026-03-26T10:00:00Z"))
                .unwrap();
            r.insert_token_event(&token_ev_at("s1", 100, 50, 0, 3.0, "2026-03-27T10:00:00Z"))
                .unwrap();
            r.insert_token_event(&token_ev_at("s2", 100, 50, 0, 7.0, "2026-03-27T14:00:00Z"))
                .unwrap();
            r.insert_token_event(&token_ev_at("s1", 100, 50, 0, 4.0, "2026-03-28T10:00:00Z"))
                .unwrap();

            let points = r.daily_cost_per_session(30, None).unwrap();
            assert_eq!(points.len(), 3);
            // day1: $5 / 1 session = $5
            assert!((points[0].value - 5.0).abs() < 1e-9);
            // day2: ($3+$7) / 2 sessions = $5
            assert!((points[1].value - 5.0).abs() < 1e-9);
            // day3: $4 / 1 session = $4
            assert!((points[2].value - 4.0).abs() < 1e-9);
        });
    }

    #[test]
    fn daily_cost_per_session_empty_when_no_events() {
        let r = repo();
        r.with_rollback(|r| {
            let points = r.daily_cost_per_session(30, None).unwrap();
            assert!(points.is_empty());
        });
    }

    #[test]
    fn daily_cache_hit_ratio_computes_correctly() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            // day1: input=100, cache_read=900 → ratio = 900/(100+900) = 0.9
            r.insert_token_event(&token_ev_at(
                "s1",
                100,
                50,
                900,
                1.0,
                "2026-03-27T10:00:00Z",
            ))
            .unwrap();
            // day2: input=500, cache_read=500 → ratio = 500/(500+500) = 0.5
            r.insert_token_event(&token_ev_at(
                "s1",
                500,
                50,
                500,
                1.0,
                "2026-03-28T10:00:00Z",
            ))
            .unwrap();

            let points = r.daily_cache_hit_ratio(30, None).unwrap();
            assert_eq!(points.len(), 2);
            assert!((points[0].value - 0.9).abs() < 1e-9);
            assert!((points[1].value - 0.5).abs() < 1e-9);
        });
    }

    #[test]
    fn daily_tool_error_rates_groups_by_tool() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            // Grep: day1 = 2/10 = 0.2, day2 = 1/5 = 0.2
            for _ in 0..8 {
                r.insert_tool_call(&tool_call_at("s1", "Grep", false, "2026-03-27T10:00:00Z"))
                    .unwrap();
            }
            for _ in 0..2 {
                r.insert_tool_call(&tool_call_at("s1", "Grep", true, "2026-03-27T10:00:00Z"))
                    .unwrap();
            }
            for _ in 0..4 {
                r.insert_tool_call(&tool_call_at("s1", "Grep", false, "2026-03-28T10:00:00Z"))
                    .unwrap();
            }
            r.insert_tool_call(&tool_call_at("s1", "Grep", true, "2026-03-28T10:00:00Z"))
                .unwrap();

            let result = r.daily_tool_error_rates(30, None).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].0, "Grep");
            assert_eq!(result[0].1.len(), 2);
            assert!((result[0].1[0].value - 0.2).abs() < 1e-9);
            assert!((result[0].1[1].value - 0.2).abs() < 1e-9);
        });
    }

    #[test]
    fn load_summary_model_counts_groups_by_model() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            // Sonnet event
            let mut ev = token_ev("s1", 100, 50, 0.5);
            ev.model = Some("claude-sonnet-4-20250514".to_string());
            r.insert_token_event(&ev).unwrap();
            // Opus event
            let mut ev2 = token_ev("s1", 3000, 1500, 5.0);
            ev2.model = Some("claude-opus-4-20250514".to_string());
            r.insert_token_event(&ev2).unwrap();

            let summary = r.load_summary().unwrap();
            assert_eq!(summary.model_counts.len(), 2);
            // Sorted by cost DESC → opus first
            assert_eq!(summary.model_counts[0].model, "claude-opus-4-20250514");
            assert_eq!(summary.model_counts[0].input_tokens, 3000);
            assert!((summary.model_counts[0].cost_usd - 5.0).abs() < 1e-9);
            assert_eq!(summary.model_counts[1].model, "claude-sonnet-4-20250514");
            assert_eq!(summary.model_counts[1].input_tokens, 100);
        });
    }

    #[test]
    fn load_summary_model_counts_empty_when_no_events() {
        let r = repo();
        r.with_rollback(|r| {
            let summary = r.load_summary().unwrap();
            assert!(summary.model_counts.is_empty());
        });
    }

    // ── AnalyticsPort テスト (#13) ──────────────────────────────

    #[test]
    fn tool_usage_sequences_detects_pairs() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            // 順序: Read → Edit → Read → Edit → Bash
            r.insert_tool_call(&tool_call_at("s1", "Read", false, "2026-01-01T00:00:01Z"))
                .unwrap();
            r.insert_tool_call(&tool_call_at("s1", "Edit", false, "2026-01-01T00:00:02Z"))
                .unwrap();
            r.insert_tool_call(&tool_call_at("s1", "Read", false, "2026-01-01T00:00:03Z"))
                .unwrap();
            r.insert_tool_call(&tool_call_at("s1", "Edit", false, "2026-01-01T00:00:04Z"))
                .unwrap();
            r.insert_tool_call(&tool_call_at("s1", "Bash", false, "2026-01-01T00:00:05Z"))
                .unwrap();

            let seqs = r.tool_usage_sequences(10).unwrap();
            assert!(!seqs.is_empty());
            // Read→Edit が最も多い (2回)
            let top = &seqs[0];
            assert_eq!(top.tool_a, "Read");
            assert_eq!(top.tool_b, "Edit");
            assert_eq!(top.count, 2);
        });
    }

    #[test]
    fn tool_usage_sequences_empty_when_no_calls() {
        let r = repo();
        r.with_rollback(|r| {
            let seqs = r.tool_usage_sequences(10).unwrap();
            assert!(seqs.is_empty());
        });
    }

    #[test]
    fn model_switching_patterns_detects_switches() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            let mut ev1 = token_ev_at("s1", 100, 50, 0, 1.0, "2026-01-01T00:00:01Z");
            ev1.model = Some("claude-sonnet-4-6".to_string());
            r.insert_token_event(&ev1).unwrap();
            let mut ev2 = token_ev_at("s1", 100, 50, 0, 5.0, "2026-01-01T00:00:02Z");
            ev2.model = Some("claude-opus-4-6".to_string());
            r.insert_token_event(&ev2).unwrap();
            let mut ev3 = token_ev_at("s1", 100, 50, 0, 1.0, "2026-01-01T00:00:03Z");
            ev3.model = Some("claude-sonnet-4-6".to_string());
            r.insert_token_event(&ev3).unwrap();

            let switches = r.model_switching_patterns().unwrap();
            assert_eq!(switches.len(), 2);
        });
    }

    #[test]
    fn model_switching_patterns_empty_when_no_switches() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            r.insert_token_event(&token_ev("s1", 100, 50, 1.0)).unwrap();
            r.insert_token_event(&token_ev("s1", 200, 50, 2.0)).unwrap();

            let switches = r.model_switching_patterns().unwrap();
            assert!(switches.is_empty());
        });
    }

    #[test]
    fn hourly_efficiency_groups_by_hour() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            r.insert_token_event(&token_ev_at("s1", 100, 50, 0, 2.0, "2026-01-01T10:00:00Z"))
                .unwrap();
            r.insert_token_event(&token_ev_at("s1", 200, 80, 0, 4.0, "2026-01-01T14:00:00Z"))
                .unwrap();

            let hours = r.hourly_efficiency().unwrap();
            assert_eq!(hours.len(), 2);
            assert_eq!(hours[0].hour, 10);
            assert_eq!(hours[1].hour, 14);
        });
    }

    // ── OptimizationPort テスト (#14) ────────────────────────────

    #[test]
    fn find_overprovisioned_sessions_filters_opus() {
        let r = repo();
        r.with_rollback(|r| {
            let mut s1 = session("s1", "proj", true);
            s1.model = Some("claude-opus-4-6".to_string());
            r.upsert_session(&s1).unwrap();
            let mut s2 = session("s2", "proj", true);
            s2.model = Some("claude-sonnet-4-6".to_string());
            r.upsert_session(&s2).unwrap();

            r.insert_token_event(&token_ev("s1", 100, 50, 5.0)).unwrap();
            r.insert_token_event(&token_ev("s2", 100, 50, 1.0)).unwrap();

            let profiles = r.find_overprovisioned_sessions(None).unwrap();
            assert_eq!(profiles.len(), 1, "only opus sessions");
            assert_eq!(profiles[0].session_id, "s1");
        });
    }

    #[test]
    fn find_overprovisioned_sessions_empty_when_no_opus() {
        let r = repo();
        r.with_rollback(|r| {
            r.upsert_session(&session("s1", "proj", true)).unwrap();
            r.insert_token_event(&token_ev("s1", 100, 50, 1.0)).unwrap();

            let profiles = r.find_overprovisioned_sessions(None).unwrap();
            assert!(profiles.is_empty());
        });
    }

    // ── BenchmarkPort テスト (#15) ───────────────────────────────

    #[test]
    fn user_efficiency_metrics_groups_by_user() {
        let r = repo();
        r.with_rollback(|r| {
            let mut s1 = session("s1", "proj", true);
            s1.user = "alice".to_string();
            r.upsert_session(&s1).unwrap();
            let mut s2 = session("s2", "proj", true);
            s2.user = "bob".to_string();
            r.upsert_session(&s2).unwrap();

            r.insert_token_event(&token_ev("s1", 100, 50, 5.0)).unwrap();
            r.insert_token_event(&token_ev("s2", 200, 80, 2.0)).unwrap();

            let benchmarks = r.user_efficiency_metrics(None).unwrap();
            assert_eq!(benchmarks.len(), 2);
            // コスト降順: alice ($5) が先
            assert_eq!(benchmarks[0].user, "alice");
            assert!((benchmarks[0].cost_per_session - 5.0).abs() < 1e-9);
            assert_eq!(benchmarks[1].user, "bob");
        });
    }

    #[test]
    fn user_efficiency_metrics_empty_when_no_sessions() {
        let r = repo();
        r.with_rollback(|r| {
            let benchmarks = r.user_efficiency_metrics(None).unwrap();
            assert!(benchmarks.is_empty());
        });
    }
}
