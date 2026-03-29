use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::domain::{model::AnalyticsResponse, port::AnalyticsPort};

pub async fn handle(
    State(port): State<Arc<dyn AnalyticsPort>>,
) -> Result<Json<AnalyticsResponse>, StatusCode> {
    let uc = crate::application::analytics::AnalyticsUseCase::new(port);
    uc.query().map(Json).map_err(|e| {
        tracing::error!("analytics query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::sqlite::SqliteRepository;
    use axum::{body::Body, http::Request, routing::get, Router};
    use http_body_util::BodyExt;
    use std::path::Path;
    use tower::ServiceExt;

    fn app() -> Router {
        let repo = Arc::new(SqliteRepository::open(Path::new(":memory:")).unwrap());
        Router::new()
            .route("/api/analytics", get(handle))
            .with_state(repo as Arc<dyn AnalyticsPort>)
    }

    #[tokio::test]
    async fn analytics_returns_200_with_json() {
        let resp = app()
            .oneshot(Request::get("/api/analytics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("tool_sequences").is_some());
        assert!(json.get("model_switches").is_some());
        assert!(json.get("hourly_efficiency").is_some());
    }
}
