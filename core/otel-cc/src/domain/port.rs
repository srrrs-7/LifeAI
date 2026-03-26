use anyhow::Result;
use crate::domain::model::{MetricsSummary, ScanState, Session, TokenEvent, ToolCall};

/// セッション情報とスキャン状態を管理するポート
pub trait SessionPort: Send + Sync {
    fn upsert_session(&self, session: &Session) -> Result<()>;
    fn get_scan_state(&self, path: &str) -> Result<Option<ScanState>>;
    fn set_scan_state(&self, path: &str, state: &ScanState) -> Result<()>;
    fn insert_compression_event(&self, session_id: &str, timestamp: &str, summary: Option<&str>) -> Result<()>;
    fn load_summary(&self) -> Result<MetricsSummary>;
}

/// トークンイベントとツールコールを記録するポート
pub trait EventPort: Send + Sync {
    fn insert_token_event(&self, event: &TokenEvent) -> Result<()>;
    fn insert_tool_call(&self, call: &ToolCall) -> Result<()>;
}

/// OTel 生データを保存するポート
pub trait OtlpPort: Send + Sync {
    fn insert_span(
        &self,
        trace_id: Option<&str>,
        span_id: Option<&str>,
        name: Option<&str>,
        payload_json: &str,
    ) -> Result<()>;
    fn insert_metric(&self, name: Option<&str>, payload_json: &str) -> Result<()>;
    fn insert_log(&self, payload_json: &str) -> Result<()>;
}
