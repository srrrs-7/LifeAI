use std::sync::Arc;
use axum::{extract::State, response::IntoResponse};

use crate::application::metrics_cache::MetricsCache;
use crate::interface::prometheus;

pub async fn handle(State(cache): State<Arc<MetricsCache>>) -> impl IntoResponse {
    let summary = cache.snapshot().await;
    let body = prometheus::render(&summary);
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}
