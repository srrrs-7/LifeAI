use std::sync::Arc;
use tokio::sync::RwLock;

use crate::domain::model::MetricsSummary;

/// Prometheus スクレイプ用のインメモリキャッシュ
///
/// SQLite からの読み取りは比較的コストが高いため、
/// スキャン・OTLP 受信のたびに更新し、/metrics は常にこのキャッシュを返す。
pub struct MetricsCache {
    inner: Arc<RwLock<MetricsSummary>>,
}

impl MetricsCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsSummary::default())),
        }
    }

    pub async fn update(&self, summary: MetricsSummary) {
        *self.inner.write().await = summary;
    }

    pub async fn snapshot(&self) -> MetricsSummary {
        self.inner.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initial_snapshot_is_default() {
        let cache = MetricsCache::new();
        let s = cache.snapshot().await;
        assert_eq!(s.total_sessions, 0);
        assert_eq!(s.total_input_tokens, 0);
    }

    #[tokio::test]
    async fn update_reflected_in_snapshot() {
        let cache = MetricsCache::new();
        cache
            .update(MetricsSummary {
                total_sessions: 42,
                total_input_tokens: 1_000_000,
                ..Default::default()
            })
            .await;

        let s = cache.snapshot().await;
        assert_eq!(s.total_sessions, 42);
        assert_eq!(s.total_input_tokens, 1_000_000);
    }

    #[tokio::test]
    async fn multiple_updates_return_latest() {
        let cache = MetricsCache::new();
        cache
            .update(MetricsSummary {
                total_sessions: 1,
                ..Default::default()
            })
            .await;
        cache
            .update(MetricsSummary {
                total_sessions: 99,
                ..Default::default()
            })
            .await;
        assert_eq!(cache.snapshot().await.total_sessions, 99);
    }
}
