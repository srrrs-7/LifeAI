use anyhow::Result;
use std::sync::Arc;
use tracing::warn;

use crate::config::InsightThresholds;
use crate::domain::{
    model::{InsightAnnotation, InsightSeverity, MetricsSummary},
    port::{AnnotationPort, InsightStatePort, SessionPort, StatsPort, TrendDataPort},
    trend::{self, CrossingDirection},
};

pub struct InsightAnalysisUseCase {
    session_port: Arc<dyn SessionPort>,
    annotation_port: Arc<dyn AnnotationPort>,
    state_port: Arc<dyn InsightStatePort>,
    trend_port: Arc<dyn TrendDataPort>,
    stats_port: Arc<dyn StatsPort>,
    /// 同一キーを再送しない冷却期間（分）
    cooldown_minutes: i64,
    /// 外部設定可能な閾値
    thresholds: InsightThresholds,
}

impl InsightAnalysisUseCase {
    pub fn new(
        session_port: Arc<dyn SessionPort>,
        annotation_port: Arc<dyn AnnotationPort>,
        state_port: Arc<dyn InsightStatePort>,
        trend_port: Arc<dyn TrendDataPort>,
        stats_port: Arc<dyn StatsPort>,
        cooldown_minutes: i64,
        thresholds: InsightThresholds,
    ) -> Self {
        Self {
            session_port,
            annotation_port,
            state_port,
            trend_port,
            stats_port,
            cooldown_minutes,
            thresholds,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let summary = self.session_port.load_summary()?;
        let mut annotations = self.analyze(&summary);

        // 予測的インサイト（失敗しても閾値検知は継続）
        match self.analyze_trends() {
            Ok(trend_anns) => annotations.extend(trend_anns),
            Err(e) => warn!("Trend analysis failed: {e}"),
        }

        // 日次コスト予算チェック（失敗しても他のインサイトは継続）
        match self.analyze_daily_budget() {
            Ok(Some(ann)) => annotations.push(ann),
            Ok(None) => {}
            Err(e) => warn!("Daily budget check failed: {e}"),
        }

        for ann in annotations {
            match self.should_send(&ann.annotation.key, ann.count_snapshot) {
                Ok(true) => {
                    if let Err(e) = self.annotation_port.push_annotation(&ann.annotation).await {
                        warn!("Failed to push annotation '{}': {e}", ann.annotation.key);
                    } else {
                        let now = chrono::Utc::now().to_rfc3339();
                        if let Err(e) = self.state_port.upsert_insight_state(
                            &ann.annotation.key,
                            &now,
                            ann.count_snapshot,
                        ) {
                            warn!("Failed to save insight state: {e}");
                        }
                    }
                }
                Ok(false) => {}
                Err(e) => warn!(
                    "Failed to check insight state for '{}': {e}",
                    ann.annotation.key
                ),
            }
        }
        Ok(())
    }

    /// MetricsSummary を解析してアノテーション候補を返す
    fn analyze(&self, s: &MetricsSummary) -> Vec<PendingAnnotation> {
        let mut out = Vec::new();

        // Rule 1: ツール別エラー率
        for (tool, calls, errors) in &s.tool_counts {
            if *calls < self.thresholds.tool_min_calls {
                continue;
            }
            let rate = *errors as f64 / *calls as f64;
            let severity = if rate >= self.thresholds.tool_error_rate_alert {
                Some(InsightSeverity::Alert)
            } else if rate >= self.thresholds.tool_error_rate_warn {
                Some(InsightSeverity::Warn)
            } else {
                None
            };
            if let Some(sev) = severity {
                out.push(PendingAnnotation {
                    annotation: InsightAnnotation {
                        key: format!("tool_error_rate:{tool}"),
                        severity: sev,
                        text: format!(
                            "ツール {tool} のエラー率が {:.1}% です（{errors}/{calls} 回）",
                            rate * 100.0
                        ),
                        tags: vec!["otel-cc".into(), "tool-error".into(), tool.to_lowercase()],
                    },
                    count_snapshot: *errors,
                });
            }
        }

        // Rule 2: キャッシュヒット率
        let total_input = s.total_input_tokens + s.total_cache_read_tokens;
        if total_input > 0 {
            let ratio = s.total_cache_read_tokens as f64 / total_input as f64;
            let severity = if ratio < self.thresholds.cache_hit_ratio_alert {
                Some(InsightSeverity::Alert)
            } else if ratio < self.thresholds.cache_hit_ratio_warn {
                Some(InsightSeverity::Warn)
            } else {
                None
            };
            if let Some(sev) = severity {
                out.push(PendingAnnotation {
                    annotation: InsightAnnotation {
                        key: "cache_hit_ratio".into(),
                        severity: sev,
                        text: format!(
                            "キャッシュヒット率が {:.1}% に低下しています",
                            ratio * 100.0
                        ),
                        tags: vec!["otel-cc".into(), "cache".into()],
                    },
                    count_snapshot: 0,
                });
            }
        }

        // Rule 3: セッションあたりコスト
        if s.total_sessions > 0 {
            let cost_per = s.total_cost_usd / s.total_sessions as f64;
            let severity = if cost_per >= self.thresholds.cost_per_session_alert {
                Some(InsightSeverity::Alert)
            } else if cost_per >= self.thresholds.cost_per_session_warn {
                Some(InsightSeverity::Warn)
            } else {
                None
            };
            if let Some(sev) = severity {
                out.push(PendingAnnotation {
                    annotation: InsightAnnotation {
                        key: "cost_per_session".into(),
                        severity: sev,
                        text: format!(
                            "セッションあたりコストが ${cost_per:.2} です（総計 ${:.2} / {}セッション）",
                            s.total_cost_usd, s.total_sessions
                        ),
                        tags: vec!["otel-cc".into(), "cost".into()],
                    },
                    count_snapshot: 0,
                });
            }
        }

        // Rule 4: コンテキスト圧縮（増分検知）
        if s.total_compression_events > 0 {
            out.push(PendingAnnotation {
                annotation: InsightAnnotation {
                    key: "compression_events".into(),
                    severity: InsightSeverity::Info,
                    text: format!(
                        "コンテキスト圧縮を検出しました（累計 {} 件）",
                        s.total_compression_events
                    ),
                    tags: vec!["otel-cc".into(), "compression".into()],
                },
                count_snapshot: s.total_compression_events,
            });
        }

        out
    }

    /// クールダウン期間内の再送を防ぐ。compression events は count 増分でも送信。
    fn should_send(&self, key: &str, current_count: i64) -> Result<bool> {
        match self.state_port.get_insight_state(key)? {
            None => Ok(true),
            Some(state) => {
                // compression events: 件数が増えていれば送信
                if key == "compression_events" && current_count > state.last_count {
                    return Ok(true);
                }
                // クールダウン判定
                let last = chrono::DateTime::parse_from_rfc3339(&state.last_sent_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or(chrono::DateTime::<chrono::Utc>::MIN_UTC);
                let elapsed = chrono::Utc::now().signed_duration_since(last);
                Ok(elapsed.num_minutes() >= self.cooldown_minutes)
            }
        }
    }

    /// トレンド分析による予測的インサイトを生成
    fn analyze_trends(&self) -> Result<Vec<PendingAnnotation>> {
        let mut out = Vec::new();

        // P1: セッション単価の上昇トレンド
        let cost_points = self
            .trend_port
            .daily_cost_per_session(self.thresholds.trend_lookback_days, None)?;
        if let Some(t) = trend::linear_regression(&cost_points) {
            // Warn: 7日以内に $10 超え予測
            if let Some(days) = trend::days_until_crossing(
                &t,
                self.thresholds.cost_per_session_warn,
                CrossingDirection::Rising,
            ) {
                if days <= self.thresholds.trend_prediction_horizon_days {
                    let projected = t.current_value + t.slope_per_day * days;
                    out.push(PendingAnnotation {
                        annotation: InsightAnnotation {
                            key: "predict:cost_per_session:warn".into(),
                            severity: InsightSeverity::Warn,
                            text: format!(
                                "コスト上昇トレンド — {days:.1}日後に ${projected:.2}/session に到達予測（現在: ${:.2}/session）",
                                t.current_value
                            ),
                            tags: vec!["otel-cc".into(), "predictive".into(), "cost".into()],
                        },
                        count_snapshot: 0,
                    });
                }
            }
            // Alert: 7日以内に $15 超え予測
            if let Some(days) = trend::days_until_crossing(
                &t,
                self.thresholds.cost_per_session_alert,
                CrossingDirection::Rising,
            ) {
                if days <= self.thresholds.trend_prediction_horizon_days {
                    let projected = t.current_value + t.slope_per_day * days;
                    out.push(PendingAnnotation {
                        annotation: InsightAnnotation {
                            key: "predict:cost_per_session:alert".into(),
                            severity: InsightSeverity::Alert,
                            text: format!(
                                "コスト急上昇 — {days:.1}日後に ${projected:.2}/session に到達予測（現在: ${:.2}/session）",
                                t.current_value
                            ),
                            tags: vec!["otel-cc".into(), "predictive".into(), "cost".into()],
                        },
                        count_snapshot: 0,
                    });
                }
            }
        }

        // P2: キャッシュヒット率の低下トレンド
        let cache_points = self
            .trend_port
            .daily_cache_hit_ratio(self.thresholds.trend_lookback_days, None)?;
        if let Some(t) = trend::linear_regression(&cache_points) {
            // Warn: 7日以内に 90% 割れ予測
            if let Some(days) = trend::days_until_crossing(
                &t,
                self.thresholds.cache_hit_ratio_warn,
                CrossingDirection::Falling,
            ) {
                if days <= self.thresholds.trend_prediction_horizon_days {
                    let projected = t.current_value + t.slope_per_day * days;
                    out.push(PendingAnnotation {
                        annotation: InsightAnnotation {
                            key: "predict:cache_hit_ratio:warn".into(),
                            severity: InsightSeverity::Warn,
                            text: format!(
                                "キャッシュ率低下トレンド — {days:.1}日後に {:.1}% に低下予測（現在: {:.1}%）",
                                projected * 100.0, t.current_value * 100.0
                            ),
                            tags: vec!["otel-cc".into(), "predictive".into(), "cache".into()],
                        },
                        count_snapshot: 0,
                    });
                }
            }
            // Alert: 7日以内に 50% 割れ予測
            if let Some(days) = trend::days_until_crossing(
                &t,
                self.thresholds.cache_hit_ratio_alert,
                CrossingDirection::Falling,
            ) {
                if days <= self.thresholds.trend_prediction_horizon_days {
                    let projected = t.current_value + t.slope_per_day * days;
                    out.push(PendingAnnotation {
                        annotation: InsightAnnotation {
                            key: "predict:cache_hit_ratio:alert".into(),
                            severity: InsightSeverity::Alert,
                            text: format!(
                                "キャッシュ率急低下 — {days:.1}日後に {:.1}% に低下予測（現在: {:.1}%）",
                                projected * 100.0, t.current_value * 100.0
                            ),
                            tags: vec!["otel-cc".into(), "predictive".into(), "cache".into()],
                        },
                        count_snapshot: 0,
                    });
                }
            }
        }

        // P3: ツール別エラー率の上昇トレンド
        let tool_rates = self
            .trend_port
            .daily_tool_error_rates(self.thresholds.trend_lookback_days, None)?;
        for (tool_name, points) in &tool_rates {
            if let Some(t) = trend::linear_regression(points) {
                // Warn: 7日以内に 5% 超え予測
                if let Some(days) = trend::days_until_crossing(
                    &t,
                    self.thresholds.tool_error_rate_warn,
                    CrossingDirection::Rising,
                ) {
                    if days <= self.thresholds.trend_prediction_horizon_days {
                        let projected = t.current_value + t.slope_per_day * days;
                        out.push(PendingAnnotation {
                            annotation: InsightAnnotation {
                                key: format!("predict:tool_error_rate:{tool_name}:warn"),
                                severity: InsightSeverity::Warn,
                                text: format!(
                                    "ツール {tool_name} エラー率上昇トレンド — {days:.1}日後に {:.1}% に到達予測（現在: {:.1}%）",
                                    projected * 100.0, t.current_value * 100.0
                                ),
                                tags: vec!["otel-cc".into(), "predictive".into(), "tool-error".into(), tool_name.to_lowercase()],
                            },
                            count_snapshot: 0,
                        });
                    }
                }
                // Alert: 7日以内に 10% 超え予測
                if let Some(days) = trend::days_until_crossing(
                    &t,
                    self.thresholds.tool_error_rate_alert,
                    CrossingDirection::Rising,
                ) {
                    if days <= self.thresholds.trend_prediction_horizon_days {
                        let projected = t.current_value + t.slope_per_day * days;
                        out.push(PendingAnnotation {
                            annotation: InsightAnnotation {
                                key: format!("predict:tool_error_rate:{tool_name}:alert"),
                                severity: InsightSeverity::Alert,
                                text: format!(
                                    "ツール {tool_name} エラー率急上昇 — {days:.1}日後に {:.1}% に到達予測（現在: {:.1}%）",
                                    projected * 100.0, t.current_value * 100.0
                                ),
                                tags: vec!["otel-cc".into(), "predictive".into(), "tool-error".into(), tool_name.to_lowercase()],
                            },
                            count_snapshot: 0,
                        });
                    }
                }
            }
        }

        Ok(out)
    }

    /// 直近24時間のコストが日次上限を超えているか確認し、超過時にアノテーションを返す
    fn analyze_daily_budget(&self) -> Result<Option<PendingAnnotation>> {
        let stats = self.stats_port.query_stats(Some(1), None, None)?;
        let daily_cost = stats.overview.cost_usd;
        if daily_cost >= self.thresholds.daily_cost_usd_alert {
            Ok(Some(PendingAnnotation {
                annotation: InsightAnnotation {
                    key: "daily_cost_budget".into(),
                    severity: InsightSeverity::Alert,
                    text: format!(
                        "本日のコストが ${daily_cost:.2} に達しました（上限: ${:.2}）",
                        self.thresholds.daily_cost_usd_alert
                    ),
                    tags: vec!["otel-cc".into(), "cost".into(), "budget".into()],
                },
                count_snapshot: 0,
            }))
        } else {
            Ok(None)
        }
    }
}

/// analyze() の内部型（アノテーション + 状態保存用カウント）
struct PendingAnnotation {
    annotation: InsightAnnotation,
    count_snapshot: i64,
}

// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        model::{
            DailyDataPoint, InsightAnnotation, InsightState, MetricsSummary, OverviewStats,
            ScanState, Session, StatsResponse,
        },
        port::{AnnotationPort, InsightStatePort, SessionPort, StatsPort, TrendDataPort},
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // ── Mock: SessionPort ─────────────────────────────────────────

    struct MockSession(MetricsSummary);

    impl SessionPort for MockSession {
        fn upsert_session(&self, _: &Session) -> Result<()> {
            Ok(())
        }
        fn get_scan_state(&self, _: &str) -> Result<Option<ScanState>> {
            Ok(None)
        }
        fn set_scan_state(&self, _: &str, _: &ScanState) -> Result<()> {
            Ok(())
        }
        fn insert_compression_event(&self, _: &str, _: &str, _: Option<&str>) -> Result<()> {
            Ok(())
        }
        fn load_summary(&self) -> Result<MetricsSummary> {
            Ok(self.0.clone())
        }
    }

    // ── Mock: AnnotationPort ──────────────────────────────────────

    #[derive(Default)]
    struct MockAnnotation {
        sent: Mutex<Vec<InsightAnnotation>>,
    }

    #[async_trait]
    impl AnnotationPort for MockAnnotation {
        async fn push_annotation(&self, ann: &InsightAnnotation) -> Result<()> {
            self.sent.lock().unwrap().push(ann.clone());
            Ok(())
        }
    }

    // ── Mock: InsightStatePort ────────────────────────────────────

    #[derive(Default)]
    struct MockInsightState {
        states: Mutex<std::collections::HashMap<String, InsightState>>,
    }

    impl InsightStatePort for MockInsightState {
        fn get_insight_state(&self, key: &str) -> Result<Option<InsightState>> {
            Ok(self.states.lock().unwrap().get(key).cloned())
        }
        fn upsert_insight_state(&self, key: &str, sent_at: &str, count: i64) -> Result<()> {
            self.states.lock().unwrap().insert(
                key.to_string(),
                InsightState {
                    key: key.to_string(),
                    last_sent_at: sent_at.to_string(),
                    last_count: count,
                },
            );
            Ok(())
        }
    }

    // ── Mock: StatsPort ──────────────────────────────────────────────

    struct MockStats(f64); // 直近24時間のコスト (USD)

    impl StatsPort for MockStats {
        fn query_stats(
            &self,
            _period: Option<u32>,
            _project: Option<&str>,
            _user: Option<&str>,
        ) -> Result<StatsResponse> {
            Ok(StatsResponse {
                overview: OverviewStats {
                    cost_usd: self.0,
                    ..Default::default()
                },
                ..Default::default()
            })
        }
    }

    // ── Mock: TrendDataPort ─────────────────────────────────────────

    #[derive(Default)]
    struct MockTrendData {
        cost: Vec<DailyDataPoint>,
        cache: Vec<DailyDataPoint>,
        tools: Vec<(String, Vec<DailyDataPoint>)>,
    }

    impl TrendDataPort for MockTrendData {
        fn daily_cost_per_session(
            &self,
            _: u32,
            _user: Option<&str>,
        ) -> Result<Vec<DailyDataPoint>> {
            Ok(self.cost.clone())
        }
        fn daily_cache_hit_ratio(
            &self,
            _: u32,
            _user: Option<&str>,
        ) -> Result<Vec<DailyDataPoint>> {
            Ok(self.cache.clone())
        }
        fn daily_tool_error_rates(
            &self,
            _: u32,
            _user: Option<&str>,
        ) -> Result<Vec<(String, Vec<DailyDataPoint>)>> {
            Ok(self.tools.clone())
        }
    }

    fn default_thresholds() -> InsightThresholds {
        InsightThresholds {
            tool_error_rate_warn: 0.05,
            tool_error_rate_alert: 0.10,
            tool_min_calls: 5,
            cache_hit_ratio_warn: 0.90,
            cache_hit_ratio_alert: 0.50,
            cost_per_session_warn: 10.0,
            cost_per_session_alert: 15.0,
            daily_cost_usd_alert: 10.0,
            trend_lookback_days: 14,
            trend_prediction_horizon_days: 7.0,
        }
    }

    fn make_uc(
        summary: MetricsSummary,
        annotation: Arc<MockAnnotation>,
        state: Arc<MockInsightState>,
        cooldown: i64,
    ) -> InsightAnalysisUseCase {
        make_uc_with_trends(
            summary,
            annotation,
            state,
            cooldown,
            MockTrendData::default(),
        )
    }

    fn make_uc_with_trends(
        summary: MetricsSummary,
        annotation: Arc<MockAnnotation>,
        state: Arc<MockInsightState>,
        cooldown: i64,
        trend_data: MockTrendData,
    ) -> InsightAnalysisUseCase {
        InsightAnalysisUseCase::new(
            Arc::new(MockSession(summary)),
            annotation,
            state,
            Arc::new(trend_data),
            Arc::new(MockStats(0.0)),
            cooldown,
            default_thresholds(),
        )
    }

    // ── ツールエラー率 ──────────────────────────────────────────

    #[tokio::test]
    async fn tool_error_rate_alert_triggers_annotation() {
        let summary = MetricsSummary {
            tool_counts: vec![("Grep".to_string(), 22, 2)], // 9.1% → Warn
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let state = Arc::new(MockInsightState::default());
        let uc = make_uc(summary, ann.clone(), state, 60);
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].key, "tool_error_rate:Grep");
        assert_eq!(sent[0].severity, InsightSeverity::Warn);
    }

    #[tokio::test]
    async fn tool_error_rate_above_10pct_is_alert() {
        let summary = MetricsSummary {
            tool_counts: vec![("Glob".to_string(), 10, 2)], // 20% → Alert
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert_eq!(sent[0].severity, InsightSeverity::Alert);
    }

    #[tokio::test]
    async fn tool_error_rate_below_5pct_no_annotation() {
        let summary = MetricsSummary {
            tool_counts: vec![("Read".to_string(), 100, 4)], // 4% → no annotation
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        assert!(ann.sent.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn tool_min_calls_filter_skips_low_volume() {
        let summary = MetricsSummary {
            tool_counts: vec![("Rare".to_string(), 2, 2)], // 100% but only 2 calls → skip
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        assert!(ann.sent.lock().unwrap().is_empty());
    }

    // ── キャッシュヒット率 ──────────────────────────────────────

    #[tokio::test]
    async fn cache_hit_ratio_warn_when_below_90pct() {
        let summary = MetricsSummary {
            total_input_tokens: 100,
            total_cache_read_tokens: 85, // 85/(100+85) = 45.9% → Alert
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(sent.iter().any(|a| a.key == "cache_hit_ratio"));
    }

    #[tokio::test]
    async fn cache_hit_ratio_no_annotation_when_tokens_zero() {
        let summary = MetricsSummary::default();
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        assert!(ann
            .sent
            .lock()
            .unwrap()
            .iter()
            .all(|a| a.key != "cache_hit_ratio"));
    }

    // ── コスト ─────────────────────────────────────────────────

    #[tokio::test]
    async fn cost_per_session_warn_at_10usd() {
        let summary = MetricsSummary {
            total_sessions: 10,
            total_cost_usd: 120.0, // $12/session → Warn
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        let cost_ann = sent.iter().find(|a| a.key == "cost_per_session").unwrap();
        assert_eq!(cost_ann.severity, InsightSeverity::Warn);
    }

    #[tokio::test]
    async fn cost_per_session_alert_at_15usd() {
        let summary = MetricsSummary {
            total_sessions: 10,
            total_cost_usd: 200.0, // $20/session → Alert
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        let cost_ann = sent.iter().find(|a| a.key == "cost_per_session").unwrap();
        assert_eq!(cost_ann.severity, InsightSeverity::Alert);
    }

    #[tokio::test]
    async fn cost_per_session_no_annotation_when_no_sessions() {
        let summary = MetricsSummary {
            total_sessions: 0,
            total_cost_usd: 500.0,
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        assert!(ann
            .sent
            .lock()
            .unwrap()
            .iter()
            .all(|a| a.key != "cost_per_session"));
    }

    // ── 圧縮イベント ────────────────────────────────────────────

    #[tokio::test]
    async fn compression_event_triggers_annotation() {
        let summary = MetricsSummary {
            total_compression_events: 3,
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(
            summary,
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
        );
        uc.run().await.unwrap();
        assert!(ann
            .sent
            .lock()
            .unwrap()
            .iter()
            .any(|a| a.key == "compression_events"));
    }

    #[tokio::test]
    async fn compression_event_resent_when_count_increases() {
        let state = Arc::new(MockInsightState::default());
        // 前回送信時 count=3
        state
            .upsert_insight_state(
                "compression_events",
                "2000-01-01T00:00:00Z", // 遠い過去でもOK（件数増加で送信）
                3,
            )
            .unwrap();

        let summary = MetricsSummary {
            total_compression_events: 5, // 増加 → 送信
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(summary, ann.clone(), state, 60 * 24 * 365); // cooldown 1年
        uc.run().await.unwrap();
        assert!(ann
            .sent
            .lock()
            .unwrap()
            .iter()
            .any(|a| a.key == "compression_events"));
    }

    // ── クールダウン ────────────────────────────────────────────

    #[tokio::test]
    async fn cooldown_prevents_duplicate_annotation() {
        let state = Arc::new(MockInsightState::default());
        // 直近に送信済み
        let just_now = chrono::Utc::now().to_rfc3339();
        state
            .upsert_insight_state("tool_error_rate:Grep", &just_now, 2)
            .unwrap();

        let summary = MetricsSummary {
            tool_counts: vec![("Grep".to_string(), 22, 2)],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(summary, ann.clone(), state, 60); // 60分クールダウン
        uc.run().await.unwrap();
        assert!(ann.sent.lock().unwrap().is_empty()); // 送信されない
    }

    #[tokio::test]
    async fn cooldown_expired_allows_resend() {
        let state = Arc::new(MockInsightState::default());
        // 2時間前に送信済み（cooldown 60分 → 期限切れ）
        let two_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        state
            .upsert_insight_state("tool_error_rate:Grep", &two_hours_ago, 2)
            .unwrap();

        let summary = MetricsSummary {
            tool_counts: vec![("Grep".to_string(), 22, 2)],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(summary, ann.clone(), state, 60);
        uc.run().await.unwrap();
        assert_eq!(ann.sent.lock().unwrap().len(), 1); // 再送される
    }

    // ── 予測的インサイト ────────────────────────────────────────

    fn dp(date: &str, value: f64) -> DailyDataPoint {
        DailyDataPoint {
            date: date.to_string(),
            value,
        }
    }

    #[tokio::test]
    async fn predict_cost_rising_triggers_warn() {
        // コストが日あたり $1 上昇中、現在 $8 → 2日後に $10 超え
        let trend_data = MockTrendData {
            cost: vec![
                dp("2026-03-25", 5.0),
                dp("2026-03-26", 6.0),
                dp("2026-03-27", 7.0),
                dp("2026-03-28", 8.0),
            ],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc_with_trends(
            MetricsSummary::default(),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
            trend_data,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(sent
            .iter()
            .any(|a| a.key == "predict:cost_per_session:warn"));
    }

    #[tokio::test]
    async fn predict_cost_rising_triggers_alert_at_15() {
        // コストが日あたり $2 上昇中、現在 $12 → 1.5日後に $15 超え
        let trend_data = MockTrendData {
            cost: vec![
                dp("2026-03-25", 6.0),
                dp("2026-03-26", 8.0),
                dp("2026-03-27", 10.0),
                dp("2026-03-28", 12.0),
            ],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc_with_trends(
            MetricsSummary::default(),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
            trend_data,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(sent
            .iter()
            .any(|a| a.key == "predict:cost_per_session:alert"));
    }

    #[tokio::test]
    async fn predict_no_annotation_for_flat_cost() {
        let trend_data = MockTrendData {
            cost: vec![
                dp("2026-03-25", 5.0),
                dp("2026-03-26", 5.0),
                dp("2026-03-27", 5.0),
                dp("2026-03-28", 5.0),
            ],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc_with_trends(
            MetricsSummary::default(),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
            trend_data,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(!sent.iter().any(|a| a.key.starts_with("predict:")));
    }

    #[tokio::test]
    async fn predict_no_annotation_for_insufficient_data() {
        // 2ポイントしかない → 回帰スキップ
        let trend_data = MockTrendData {
            cost: vec![dp("2026-03-27", 8.0), dp("2026-03-28", 9.0)],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc_with_trends(
            MetricsSummary::default(),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
            trend_data,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(!sent.iter().any(|a| a.key.starts_with("predict:")));
    }

    #[tokio::test]
    async fn predict_cache_falling_triggers_warn() {
        // キャッシュ率が日あたり -1% 低下中、現在 92% → 2日後に 90% 割れ
        let trend_data = MockTrendData {
            cache: vec![
                dp("2026-03-25", 0.95),
                dp("2026-03-26", 0.94),
                dp("2026-03-27", 0.93),
                dp("2026-03-28", 0.92),
            ],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc_with_trends(
            MetricsSummary::default(),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
            trend_data,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(sent.iter().any(|a| a.key == "predict:cache_hit_ratio:warn"));
    }

    #[tokio::test]
    async fn predict_tool_error_rising_triggers_warn() {
        // Grep のエラー率が日あたり +0.5% 上昇中、現在 3% → 4日後に 5% 超え
        let trend_data = MockTrendData {
            tools: vec![(
                "Grep".to_string(),
                vec![
                    dp("2026-03-25", 0.015),
                    dp("2026-03-26", 0.02),
                    dp("2026-03-27", 0.025),
                    dp("2026-03-28", 0.03),
                ],
            )],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc_with_trends(
            MetricsSummary::default(),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
            trend_data,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(sent
            .iter()
            .any(|a| a.key == "predict:tool_error_rate:Grep:warn"));
    }

    #[tokio::test]
    async fn predict_cost_beyond_horizon_no_annotation() {
        // コストが日あたり $0.2 上昇中、現在 $5 → $10 まで25日 → 7日horizon外
        let trend_data = MockTrendData {
            cost: vec![
                dp("2026-03-25", 4.4),
                dp("2026-03-26", 4.6),
                dp("2026-03-27", 4.8),
                dp("2026-03-28", 5.0),
            ],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc_with_trends(
            MetricsSummary::default(),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            60,
            trend_data,
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(!sent.iter().any(|a| a.key.starts_with("predict:")));
    }

    // ── 日次コスト予算アラート ──────────────────────────────────────

    #[tokio::test]
    async fn daily_cost_above_alert_triggers_annotation() {
        let ann = Arc::new(MockAnnotation::default());
        let uc = InsightAnalysisUseCase::new(
            Arc::new(MockSession(MetricsSummary::default())),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            Arc::new(MockTrendData::default()),
            Arc::new(MockStats(12.5)), // $12.50 > $10 上限
            60,
            default_thresholds(),
        );
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        let budget_ann = sent.iter().find(|a| a.key == "daily_cost_budget");
        assert!(budget_ann.is_some());
        assert_eq!(budget_ann.unwrap().severity, InsightSeverity::Alert);
    }

    #[tokio::test]
    async fn daily_cost_below_alert_no_annotation() {
        let ann = Arc::new(MockAnnotation::default());
        let uc = InsightAnalysisUseCase::new(
            Arc::new(MockSession(MetricsSummary::default())),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            Arc::new(MockTrendData::default()),
            Arc::new(MockStats(8.0)), // $8.00 < $10 上限
            60,
            default_thresholds(),
        );
        uc.run().await.unwrap();
        assert!(!ann
            .sent
            .lock()
            .unwrap()
            .iter()
            .any(|a| a.key == "daily_cost_budget"));
    }

    #[tokio::test]
    async fn daily_cost_exactly_at_threshold_triggers_annotation() {
        let ann = Arc::new(MockAnnotation::default());
        let uc = InsightAnalysisUseCase::new(
            Arc::new(MockSession(MetricsSummary::default())),
            ann.clone(),
            Arc::new(MockInsightState::default()),
            Arc::new(MockTrendData::default()),
            Arc::new(MockStats(10.0)), // ちょうど上限 = アラート対象
            60,
            default_thresholds(),
        );
        uc.run().await.unwrap();
        assert!(ann
            .sent
            .lock()
            .unwrap()
            .iter()
            .any(|a| a.key == "daily_cost_budget"));
    }
}
