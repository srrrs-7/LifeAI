use anyhow::Result;
use std::sync::Arc;
use tracing::warn;

use crate::domain::{
    model::{InsightAnnotation, InsightSeverity, MetricsSummary},
    port::{AnnotationPort, InsightStatePort, SessionPort},
};

/// 閾値定数
const TOOL_ERROR_RATE_WARN: f64 = 0.05;
const TOOL_ERROR_RATE_ALERT: f64 = 0.10;
const TOOL_MIN_CALLS: i64 = 5;
const CACHE_HIT_RATIO_WARN: f64 = 0.90;
const CACHE_HIT_RATIO_ALERT: f64 = 0.50;
const COST_PER_SESSION_WARN: f64 = 10.0;
const COST_PER_SESSION_ALERT: f64 = 15.0;

pub struct InsightAnalysisUseCase {
    session_port: Arc<dyn SessionPort>,
    annotation_port: Arc<dyn AnnotationPort>,
    state_port: Arc<dyn InsightStatePort>,
    /// 同一キーを再送しない冷却期間（分）
    cooldown_minutes: i64,
}

impl InsightAnalysisUseCase {
    pub fn new(
        session_port: Arc<dyn SessionPort>,
        annotation_port: Arc<dyn AnnotationPort>,
        state_port: Arc<dyn InsightStatePort>,
        cooldown_minutes: i64,
    ) -> Self {
        Self {
            session_port,
            annotation_port,
            state_port,
            cooldown_minutes,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let summary = self.session_port.load_summary()?;
        let annotations = self.analyze(&summary);

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
                Err(e) => warn!("Failed to check insight state for '{}': {e}", ann.annotation.key),
            }
        }
        Ok(())
    }

    /// MetricsSummary を解析してアノテーション候補を返す
    fn analyze(&self, s: &MetricsSummary) -> Vec<PendingAnnotation> {
        let mut out = Vec::new();

        // Rule 1: ツール別エラー率
        for (tool, calls, errors) in &s.tool_counts {
            if *calls < TOOL_MIN_CALLS {
                continue;
            }
            let rate = *errors as f64 / *calls as f64;
            let severity = if rate >= TOOL_ERROR_RATE_ALERT {
                Some(InsightSeverity::Alert)
            } else if rate >= TOOL_ERROR_RATE_WARN {
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
                        tags: vec![
                            "otel-cc".into(),
                            "tool-error".into(),
                            tool.to_lowercase(),
                        ],
                    },
                    count_snapshot: *errors,
                });
            }
        }

        // Rule 2: キャッシュヒット率
        let total_input = s.total_input_tokens + s.total_cache_read_tokens;
        if total_input > 0 {
            let ratio = s.total_cache_read_tokens as f64 / total_input as f64;
            let severity = if ratio < CACHE_HIT_RATIO_ALERT {
                Some(InsightSeverity::Alert)
            } else if ratio < CACHE_HIT_RATIO_WARN {
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
            let severity = if cost_per >= COST_PER_SESSION_ALERT {
                Some(InsightSeverity::Alert)
            } else if cost_per >= COST_PER_SESSION_WARN {
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
        model::{InsightAnnotation, InsightState, MetricsSummary, ScanState, Session},
        port::{AnnotationPort, InsightStatePort, SessionPort},
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // ── Mock: SessionPort ─────────────────────────────────────────

    struct MockSession(MetricsSummary);

    impl SessionPort for MockSession {
        fn upsert_session(&self, _: &Session) -> Result<()> { Ok(()) }
        fn get_scan_state(&self, _: &str) -> Result<Option<ScanState>> { Ok(None) }
        fn set_scan_state(&self, _: &str, _: &ScanState) -> Result<()> { Ok(()) }
        fn insert_compression_event(&self, _: &str, _: &str, _: Option<&str>) -> Result<()> { Ok(()) }
        fn load_summary(&self) -> Result<MetricsSummary> { Ok(self.0.clone()) }
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

    fn make_uc(
        summary: MetricsSummary,
        annotation: Arc<MockAnnotation>,
        state: Arc<MockInsightState>,
        cooldown: i64,
    ) -> InsightAnalysisUseCase {
        InsightAnalysisUseCase::new(
            Arc::new(MockSession(summary)),
            annotation,
            state,
            cooldown,
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
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
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
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
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
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
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
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
        uc.run().await.unwrap();
        let sent = ann.sent.lock().unwrap();
        assert!(sent.iter().any(|a| a.key == "cache_hit_ratio"));
    }

    #[tokio::test]
    async fn cache_hit_ratio_no_annotation_when_tokens_zero() {
        let summary = MetricsSummary::default();
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
        uc.run().await.unwrap();
        assert!(ann.sent.lock().unwrap().iter().all(|a| a.key != "cache_hit_ratio"));
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
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
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
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
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
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
        uc.run().await.unwrap();
        assert!(ann.sent.lock().unwrap().iter().all(|a| a.key != "cost_per_session"));
    }

    // ── 圧縮イベント ────────────────────────────────────────────

    #[tokio::test]
    async fn compression_event_triggers_annotation() {
        let summary = MetricsSummary {
            total_compression_events: 3,
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(summary, ann.clone(), Arc::new(MockInsightState::default()), 60);
        uc.run().await.unwrap();
        assert!(ann.sent.lock().unwrap().iter().any(|a| a.key == "compression_events"));
    }

    #[tokio::test]
    async fn compression_event_resent_when_count_increases() {
        let state = Arc::new(MockInsightState::default());
        // 前回送信時 count=3
        state.upsert_insight_state(
            "compression_events",
            "2000-01-01T00:00:00Z", // 遠い過去でもOK（件数増加で送信）
            3,
        ).unwrap();

        let summary = MetricsSummary {
            total_compression_events: 5, // 増加 → 送信
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(summary, ann.clone(), state, 60 * 24 * 365); // cooldown 1年
        uc.run().await.unwrap();
        assert!(ann.sent.lock().unwrap().iter().any(|a| a.key == "compression_events"));
    }

    // ── クールダウン ────────────────────────────────────────────

    #[tokio::test]
    async fn cooldown_prevents_duplicate_annotation() {
        let state = Arc::new(MockInsightState::default());
        // 直近に送信済み
        let just_now = chrono::Utc::now().to_rfc3339();
        state.upsert_insight_state("tool_error_rate:Grep", &just_now, 2).unwrap();

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
        state.upsert_insight_state("tool_error_rate:Grep", &two_hours_ago, 2).unwrap();

        let summary = MetricsSummary {
            tool_counts: vec![("Grep".to_string(), 22, 2)],
            ..Default::default()
        };
        let ann = Arc::new(MockAnnotation::default());
        let uc = make_uc(summary, ann.clone(), state, 60);
        uc.run().await.unwrap();
        assert_eq!(ann.sent.lock().unwrap().len(), 1); // 再送される
    }
}
