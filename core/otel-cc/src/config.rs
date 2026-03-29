use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// SQLite DB ファイルパス
    pub db_path: PathBuf,
    /// Claude Code ログディレクトリ (~/.claude/projects/)
    pub claude_log_dir: PathBuf,
    /// OTLP/HTTP 受信ポート
    pub otlp_port: u16,
    /// Prometheus /metrics 公開ポート
    pub metrics_port: u16,
    /// Grafana ベース URL（アノテーション送信先）
    pub grafana_url: String,
    /// インサイト分析の実行間隔（秒）
    pub insight_interval_secs: u64,
    /// 同一インサイトの再送クールダウン（分）
    pub insight_cooldown_minutes: i64,
    /// ユーザー識別名（チーム内で一意であること）
    pub user: String,
    /// インサイト閾値設定
    pub insight_thresholds: InsightThresholds,
}

/// インサイト分析の閾値（すべて環境変数で上書き可能）
#[derive(Debug, Clone)]
pub struct InsightThresholds {
    pub tool_error_rate_warn: f64,
    pub tool_error_rate_alert: f64,
    pub tool_min_calls: i64,
    pub cache_hit_ratio_warn: f64,
    pub cache_hit_ratio_alert: f64,
    pub cost_per_session_warn: f64,
    pub cost_per_session_alert: f64,
    pub trend_lookback_days: u32,
    pub trend_prediction_horizon_days: f64,
}

impl InsightThresholds {
    pub fn from_env() -> Self {
        Self {
            tool_error_rate_warn: std::env::var("OTEL_CC_INSIGHT_TOOL_ERROR_WARN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.05),
            tool_error_rate_alert: std::env::var("OTEL_CC_INSIGHT_TOOL_ERROR_ALERT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.10),
            tool_min_calls: std::env::var("OTEL_CC_INSIGHT_TOOL_MIN_CALLS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            cache_hit_ratio_warn: std::env::var("OTEL_CC_INSIGHT_CACHE_WARN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.90),
            cache_hit_ratio_alert: std::env::var("OTEL_CC_INSIGHT_CACHE_ALERT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.50),
            cost_per_session_warn: std::env::var("OTEL_CC_INSIGHT_COST_WARN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10.0),
            cost_per_session_alert: std::env::var("OTEL_CC_INSIGHT_COST_ALERT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(15.0),
            trend_lookback_days: std::env::var("OTEL_CC_INSIGHT_TREND_LOOKBACK")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(14),
            trend_prediction_horizon_days: std::env::var("OTEL_CC_INSIGHT_TREND_HORIZON")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7.0),
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            db_path: std::env::var("OTEL_CC_DB_PATH")
                .unwrap_or_else(|_| "otel-cc.db".to_string())
                .into(),
            claude_log_dir: std::env::var("OTEL_CC_CLAUDE_LOG_DIR")
                .unwrap_or_else(|_| {
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                    format!("{home}/.claude/projects")
                })
                .into(),
            otlp_port: std::env::var("OTEL_CC_OTLP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4318),
            metrics_port: std::env::var("OTEL_CC_METRICS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9091),
            grafana_url: std::env::var("OTEL_CC_GRAFANA_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            insight_interval_secs: std::env::var("OTEL_CC_INSIGHT_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            insight_cooldown_minutes: std::env::var("OTEL_CC_INSIGHT_COOLDOWN_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            user: std::env::var("OTEL_CC_USER").unwrap_or_else(|_| whoami::username()),
            insight_thresholds: InsightThresholds::from_env(),
        }
    }
}
