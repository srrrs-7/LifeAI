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
        }
    }
}
