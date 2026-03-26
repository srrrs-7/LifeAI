use axum::{extract::State, response::IntoResponse};
use std::sync::Arc;

use crate::application::metrics_cache::MetricsCache;
use crate::interface::prometheus;

pub async fn handle(State(cache): State<Arc<MetricsCache>>) -> impl IntoResponse {
    let summary = cache.snapshot().await;
    let body = prometheus::render(&summary);
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn app() -> Router {
        let cache = Arc::new(MetricsCache::new());
        Router::new()
            .route("/metrics", get(handle))
            .with_state(cache)
    }

    #[tokio::test]
    async fn metrics_endpoint_returns_200() {
        let resp = app()
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn metrics_endpoint_content_type_is_prometheus() {
        let resp = app()
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let ct = resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(ct.contains("text/plain"), "expected text/plain, got: {ct}");
        assert!(ct.contains("0.0.4"), "expected version=0.0.4, got: {ct}");
    }

    #[tokio::test]
    async fn metrics_body_contains_help_lines() {
        let resp = app()
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("# HELP cc_sessions_total"));
    }
}
