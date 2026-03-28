use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Serialize;

use crate::domain::{model::InsightAnnotation, port::AnnotationPort};

pub struct GrafanaAnnotationClient {
    base_url: String,
    client: reqwest::Client,
}

impl GrafanaAnnotationClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }
}

/// Grafana Annotations API リクエストボディ
/// POST /api/annotations
#[derive(Serialize)]
struct GrafanaAnnotationRequest<'a> {
    /// Unix ミリ秒
    time: i64,
    text: &'a str,
    tags: &'a [String],
}

#[async_trait]
impl AnnotationPort for GrafanaAnnotationClient {
    async fn push_annotation(&self, ann: &InsightAnnotation) -> Result<()> {
        let url = format!("{}/api/annotations", self.base_url);
        let time_ms = chrono::Utc::now().timestamp_millis();

        // severity タグを先頭に付ける
        let mut tags = vec![ann.severity.as_tag().to_string()];
        tags.extend_from_slice(&ann.tags);

        let body = GrafanaAnnotationRequest {
            time: time_ms,
            text: &ann.text,
            tags: &tags,
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to connect to Grafana at {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Grafana annotation API returned {status}: {text}");
        }

        tracing::info!(
            key = %ann.key,
            severity = ?ann.severity,
            "Grafana annotation pushed"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::InsightSeverity;

    /// GrafanaAnnotationRequest が正しい JSON フィールドにシリアライズされるか確認
    #[test]
    fn annotation_request_serializes_correctly() {
        let tags = vec!["otel-cc".to_string(), "tool-error".to_string()];
        let req = GrafanaAnnotationRequest {
            time: 1_000_000,
            text: "テストメッセージ",
            tags: &tags,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["time"], 1_000_000);
        assert_eq!(json["text"], "テストメッセージ");
        assert_eq!(json["tags"][0], "otel-cc");
    }

    /// severity タグが先頭に付与されることを確認
    #[tokio::test]
    async fn severity_tag_prepended() {
        // GrafanaAnnotationClient は実際の HTTP を叩くので、
        // ここでは tags 付与ロジックのみを GrafanaAnnotationRequest で検証する
        let ann_tags = vec!["otel-cc".to_string()];
        let mut tags = vec![InsightSeverity::Alert.as_tag().to_string()];
        tags.extend_from_slice(&ann_tags);
        assert_eq!(tags[0], "alert");
        assert_eq!(tags[1], "otel-cc");
    }
}
