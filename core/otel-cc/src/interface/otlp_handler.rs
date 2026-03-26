use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Router};
use std::sync::Arc;
use tracing::warn;

use crate::application::{ingest_otlp::IngestOtlpUseCase, metrics_cache::MetricsCache};
use crate::domain::port::SessionPort;
use crate::infrastructure::otlp_reader::parser::{MetricsPayload, TracesPayload};

#[derive(Clone)]
pub struct OtlpState {
    pub use_case: Arc<IngestOtlpUseCase>,
    pub session_port: Arc<dyn SessionPort>,
    pub cache: Arc<MetricsCache>,
}

pub fn router(state: OtlpState) -> Router {
    Router::new()
        .route("/v1/traces", post(handle_traces))
        .route("/v1/metrics", post(handle_metrics))
        .route("/v1/logs", post(handle_logs))
        .with_state(state)
}

async fn handle_traces(State(state): State<OtlpState>, body: String) -> impl IntoResponse {
    let payload: TracesPayload = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            warn!("OTLP traces parse error: {e}");
            return StatusCode::BAD_REQUEST;
        }
    };

    if let Err(e) = state.use_case.ingest_traces(&payload, &body) {
        warn!("ingest_traces error: {e}");
    } else {
        refresh_cache(&state).await;
    }
    StatusCode::OK
}

async fn handle_metrics(State(state): State<OtlpState>, body: String) -> impl IntoResponse {
    let payload: MetricsPayload = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            warn!("OTLP metrics parse error: {e}");
            return StatusCode::BAD_REQUEST;
        }
    };

    if let Err(e) = state.use_case.ingest_metrics(&payload, &body) {
        warn!("ingest_metrics error: {e}");
    }
    StatusCode::OK
}

async fn handle_logs(State(state): State<OtlpState>, body: String) -> impl IntoResponse {
    if let Err(e) = state.use_case.ingest_logs(&body) {
        warn!("ingest_logs error: {e}");
    }
    StatusCode::OK
}

/// OTLP 受信後にサマリーを再計算してキャッシュを更新する
async fn refresh_cache(state: &OtlpState) {
    if let Ok(summary) = state.session_port.load_summary() {
        state.cache.update(summary).await;
    }
}
