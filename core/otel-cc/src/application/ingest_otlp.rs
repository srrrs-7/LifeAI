use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use crate::domain::{
    cost,
    model::{EventSource, Session, TokenEvent},
    port::{EventPort, OtlpPort, SessionPort},
};
use crate::infrastructure::otlp_reader::parser::{
    extract_metrics, extract_token_events, MetricsPayload, TracesPayload,
};

pub struct IngestOtlpUseCase {
    session_port: Arc<dyn SessionPort>,
    event_port: Arc<dyn EventPort>,
    otlp_port: Arc<dyn OtlpPort>,
}

impl IngestOtlpUseCase {
    pub fn new(
        session_port: Arc<dyn SessionPort>,
        event_port: Arc<dyn EventPort>,
        otlp_port: Arc<dyn OtlpPort>,
    ) -> Self {
        Self {
            session_port,
            event_port,
            otlp_port,
        }
    }

    pub fn ingest_traces(&self, payload: &TracesPayload, raw: &str) -> Result<()> {
        let events = extract_token_events(payload);
        if !events.is_empty() {
            info!("OTLP traces: {} token events", events.len());
        }

        // 生ペイロードはリクエスト単位で1件だけ保存する
        // （イベント数に関わらず raw は不変なので N 回書くのは無駄）
        let first = events.first();
        self.otlp_port.insert_span(
            first.and_then(|e| e.trace_id.as_deref()),
            first.and_then(|e| e.span_id.as_deref()),
            first.and_then(|e| e.span_name.as_deref()),
            raw,
        )?;

        let now = chrono::Utc::now().to_rfc3339();
        for ev in &events {
            // session_id がなければ trace_id を代用
            let session_id = ev
                .session_id
                .clone()
                .or_else(|| ev.trace_id.clone())
                .unwrap_or_else(|| "unknown".to_string());

            // セッションを仮登録（ログ解析結果があれば上書きされる）
            // project は OTLP 属性から取得、なければ "otlp" にフォールバック
            let project = ev.project.clone().unwrap_or_else(|| "otlp".to_string());
            let _ = self.session_port.upsert_session(&Session {
                session_id: session_id.clone(),
                project,
                cwd: None,
                git_branch: None,
                model: ev.model.clone(),
                entrypoint: Some("otlp".to_string()),
                version: None,
                started_at: now.clone(),
                last_seen_at: now.clone(),
                is_active: true,
            });

            let model_str = ev.model.as_deref().unwrap_or("claude-sonnet-4-6");
            let _ = self.event_port.insert_token_event(&TokenEvent {
                session_id,
                timestamp: now.clone(),
                model: ev.model.clone(),
                input_tokens: ev.input_tokens,
                output_tokens: ev.output_tokens,
                cache_creation_tokens: ev.cache_creation_tokens,
                cache_read_tokens: ev.cache_read_tokens,
                cost_usd: cost::calculate(
                    model_str,
                    ev.input_tokens,
                    ev.output_tokens,
                    ev.cache_creation_tokens,
                    ev.cache_read_tokens,
                ),
                source: EventSource::Otlp,
            });
        }

        Ok(())
    }

    pub fn ingest_metrics(&self, payload: &MetricsPayload, raw: &str) -> Result<()> {
        let points = extract_metrics(payload);
        if !points.is_empty() {
            info!("OTLP metrics: {} data points", points.len());
        }
        for pt in &points {
            self.otlp_port.insert_metric(Some(&pt.name), raw)?;
        }
        Ok(())
    }

    pub fn ingest_logs(&self, raw: &str) -> Result<()> {
        self.otlp_port.insert_log(raw)?;
        info!("OTLP logs received ({} bytes)", raw.len());
        Ok(())
    }
}
