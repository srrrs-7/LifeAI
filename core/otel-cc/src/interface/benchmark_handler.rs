use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::domain::{model::TeamBenchmark, port::BenchmarkPort};

#[derive(Deserialize)]
pub struct BenchmarkParams {
    pub period: Option<u32>,
}

pub async fn handle(
    State(port): State<Arc<dyn BenchmarkPort>>,
    Query(params): Query<BenchmarkParams>,
) -> Result<Json<TeamBenchmark>, StatusCode> {
    let uc = crate::application::benchmark::BenchmarkUseCase::new(port);
    uc.analyze(params.period).map(Json).map_err(|e| {
        tracing::error!("benchmark query failed: {e}");
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
            .route("/api/benchmarks", get(handle))
            .with_state(repo as Arc<dyn BenchmarkPort>)
    }

    #[tokio::test]
    async fn benchmark_returns_200_with_json() {
        let resp = app()
            .oneshot(Request::get("/api/benchmarks").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("user_benchmarks").is_some());
        assert!(json.get("best_practices").is_some());
    }
}
