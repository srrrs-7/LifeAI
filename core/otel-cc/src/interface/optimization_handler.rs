use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::domain::{model::OptimizationReport, port::OptimizationPort};

#[derive(Deserialize)]
pub struct OptParams {
    pub period: Option<u32>,
}

pub async fn handle(
    State(port): State<Arc<dyn OptimizationPort>>,
    Query(params): Query<OptParams>,
) -> Result<Json<OptimizationReport>, StatusCode> {
    let uc = crate::application::cost_optimization::CostOptimizationUseCase::new(port);
    uc.analyze(params.period).map(Json).map_err(|e| {
        tracing::error!("optimization query failed: {e}");
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
            .route("/api/optimization", get(handle))
            .with_state(repo as Arc<dyn OptimizationPort>)
    }

    #[tokio::test]
    async fn optimization_returns_200_with_json() {
        let resp = app()
            .oneshot(
                Request::get("/api/optimization")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("suggestions").is_some());
        assert!(json.get("total_potential_savings_usd").is_some());
    }
}
