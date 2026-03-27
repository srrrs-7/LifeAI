use std::fmt;

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub project: String,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub model: Option<String>,
    pub entrypoint: Option<String>,
    pub version: Option<String>,
    pub started_at: String,
    pub last_seen_at: String,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct TokenEvent {
    pub session_id: String,
    pub timestamp: String,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_usd: f64,
    pub source: EventSource,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub session_id: String,
    pub tool_id: Option<String>,
    pub timestamp: String,
    pub tool_name: String,
    pub is_error: bool,
    pub source: EventSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventSource {
    Log,
    Otlp,
}

impl fmt::Display for EventSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Log => write!(f, "log"),
            Self::Otlp => write!(f, "otlp"),
        }
    }
}

/// ファイルスキャン進捗（差分スキャン用）
#[derive(Debug, Clone)]
pub struct ScanState {
    pub last_modified: String,
    /// 処理済み行数。次回スキャン時はこの行数分スキップする
    pub lines_processed: usize,
}

/// Prometheus /metrics レンダリング用サマリー
#[derive(Debug, Default, Clone)]
pub struct MetricsSummary {
    pub total_sessions: i64,
    pub active_sessions: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_cost_usd: f64,
    pub total_tool_calls: i64,
    pub total_tool_errors: i64,
    /// コンテキスト圧縮発生回数
    pub total_compression_events: i64,
    /// (tool_name, call_count, error_count)
    pub tool_counts: Vec<(String, i64, i64)>,
    pub projects: Vec<ProjectSummary>,
    /// (entrypoint, session_count)
    pub entrypoint_counts: Vec<(String, i64)>,
}

#[derive(Debug, Clone)]
pub struct ProjectSummary {
    pub project: String,
    pub sessions: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_usd: f64,
    pub tool_calls: i64,
}

// ── /api/stats レスポンス型 ────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct StatsResponse {
    pub period_days: Option<u32>,
    pub generated_at: String,
    pub overview: OverviewStats,
    pub projects: Vec<ProjectStats>,
    pub daily: Vec<DailyStats>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct OverviewStats {
    pub total_sessions: i64,
    pub active_sessions: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_usd: f64,
    pub tool_calls: i64,
    pub tool_errors: i64,
    pub cache_hit_ratio: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectStats {
    pub project: String,
    pub sessions: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_usd: f64,
    pub tool_calls: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyStats {
    pub date: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_usd: f64,
}
