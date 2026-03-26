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
