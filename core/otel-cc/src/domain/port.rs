use crate::domain::model::{
    DailyDataPoint, HourlyEfficiency, InsightAnnotation, InsightState, MetricsSummary, ModelSwitch,
    ScanState, Session, SessionCostProfile, StatsResponse, TokenEvent, ToolCall, ToolSequence,
    UserBenchmark,
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
    /// `user`: None = 全ユーザー、Some("name") = 指定ユーザーのみ
    fn query_stats(
        &self,
        period_days: Option<u32>,
        project: Option<&str>,
        user: Option<&str>,
    ) -> Result<StatsResponse>;
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

/// トレンド分析用の日次集計データ取得ポート
pub trait TrendDataPort: Send + Sync {
    /// 直近 N 日間の日次セッション単価
    fn daily_cost_per_session(
        &self,
        lookback_days: u32,
        user: Option<&str>,
    ) -> Result<Vec<DailyDataPoint>>;
    /// 直近 N 日間の日次キャッシュヒット率
    fn daily_cache_hit_ratio(
        &self,
        lookback_days: u32,
        user: Option<&str>,
    ) -> Result<Vec<DailyDataPoint>>;
    /// 直近 N 日間のツール別日次エラー率。戻り値: Vec<(tool_name, Vec<DailyDataPoint>)>
    fn daily_tool_error_rates(
        &self,
        lookback_days: u32,
        user: Option<&str>,
    ) -> Result<Vec<(String, Vec<DailyDataPoint>)>>;
}

/// ユーザー行動パターン分析ポート (#13)
pub trait AnalyticsPort: Send + Sync {
    /// 連続して使用されるツールペアの統計（上位 limit 件）
    fn tool_usage_sequences(&self, limit: usize) -> Result<Vec<ToolSequence>>;
    /// セッション内でのモデル切り替えパターン
    fn model_switching_patterns(&self) -> Result<Vec<ModelSwitch>>;
    /// 時間帯別の効率指標
    fn hourly_efficiency(&self) -> Result<Vec<HourlyEfficiency>>;
}

/// コスト最適化分析ポート (#14)
pub trait OptimizationPort: Send + Sync {
    /// 高コストモデルを使用したセッションのプロファイル
    fn find_overprovisioned_sessions(
        &self,
        period_days: Option<u32>,
    ) -> Result<Vec<SessionCostProfile>>;
}

/// チームベンチマークポート (#15)
pub trait BenchmarkPort: Send + Sync {
    /// ユーザー別効率メトリクス
    fn user_efficiency_metrics(&self, period_days: Option<u32>) -> Result<Vec<UserBenchmark>>;
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
