use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::domain::{model::StatsResponse, port::StatsPort};

#[derive(Deserialize)]
pub struct StatsParams {
    /// 直近 N 日間のみ集計（省略時は全期間）
    pub period: Option<u32>,
    /// 特定プロジェクトのみ絞り込み（省略時は全プロジェクト）
    pub project: Option<String>,
}

pub async fn handle(
    State(port): State<Arc<dyn StatsPort>>,
    Query(params): Query<StatsParams>,
) -> Result<Json<StatsResponse>, StatusCode> {
    port.query_stats(params.period, params.project.as_deref())
        .map(Json)
        .map_err(|e| {
            tracing::error!("stats query failed: {e}");
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
            .route("/api/stats", get(handle))
            .with_state(repo as Arc<dyn StatsPort>)
    }

    #[tokio::test]
    async fn stats_returns_200_with_json() {
        let resp = app()
            .oneshot(Request::get("/api/stats").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let ct = resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(ct.contains("application/json"), "got: {ct}");
    }

    #[tokio::test]
    async fn stats_with_period_param_returns_200() {
        let resp = app()
            .oneshot(
                Request::get("/api/stats?period=7")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn stats_response_has_expected_fields() {
        let resp = app()
            .oneshot(Request::get("/api/stats").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("overview").is_some(), "missing overview");
        assert!(json.get("projects").is_some(), "missing projects");
        assert!(json.get("daily").is_some(), "missing daily");
        assert!(json.get("generated_at").is_some(), "missing generated_at");
    }
}
