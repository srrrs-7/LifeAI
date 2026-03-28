use crate::domain::model::{
    InsightAnnotation, InsightState, MetricsSummary, ScanState, Session, StatsResponse,
    TokenEvent, ToolCall,
};
use anyhow::Result;
use async_trait::async_trait;

/// セッション情報とスキャン状態を管理するポート
pub trait SessionPort: Send + Sync {
    fn upsert_session(&self, session: &Session) -> Result<()>;
    fn get_scan_state(&self, path: &str) -> Result<Option<ScanState>>;
    fn set_scan_state(&self, path: &str, state: &ScanState) -> Result<()>;
    fn insert_compression_event(
        &self,
        session_id: &str,
        timestamp: &str,
        summary: Option<&str>,
    ) -> Result<()>;
    fn load_summary(&self) -> Result<MetricsSummary>;
}

/// トークンイベントとツールコールを記録するポート
pub trait EventPort: Send + Sync {
    fn insert_token_event(&self, event: &TokenEvent) -> Result<()>;
    fn insert_tool_call(&self, call: &ToolCall) -> Result<()>;
}

/// 期間・プロジェクト指定で集計統計を返すポート
pub trait StatsPort: Send + Sync {
    /// `period_days`: None = 全期間、Some(n) = 直近 n 日
    /// `project`: None = 全プロジェクト、Some("name") = 指定プロジェクトのみ
    fn query_stats(&self, period_days: Option<u32>, project: Option<&str>)
        -> Result<StatsResponse>;
}

/// インサイト送信状態の永続化ポート（クールダウン管理）
pub trait InsightStatePort: Send + Sync {
    fn get_insight_state(&self, key: &str) -> Result<Option<InsightState>>;
    fn upsert_insight_state(&self, key: &str, sent_at: &str, count: i64) -> Result<()>;
}

/// Grafana アノテーション送信ポート（非同期 HTTP）
#[async_trait]
pub trait AnnotationPort: Send + Sync {
    async fn push_annotation(&self, ann: &InsightAnnotation) -> Result<()>;
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
