use anyhow::Result;
use std::sync::Arc;

use crate::domain::{model::AnalyticsResponse, port::AnalyticsPort};

pub struct AnalyticsUseCase {
    port: Arc<dyn AnalyticsPort>,
}

impl AnalyticsUseCase {
    pub fn new(port: Arc<dyn AnalyticsPort>) -> Self {
        Self { port }
    }

    pub fn query(&self) -> Result<AnalyticsResponse> {
        Ok(AnalyticsResponse {
            generated_at: chrono::Utc::now().to_rfc3339(),
            tool_sequences: self.port.tool_usage_sequences(20)?,
            model_switches: self.port.model_switching_patterns()?,
            hourly_efficiency: self.port.hourly_efficiency()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{HourlyEfficiency, ModelSwitch, ToolSequence};

    struct MockAnalytics;
    impl AnalyticsPort for MockAnalytics {
        fn tool_usage_sequences(&self, _limit: usize) -> Result<Vec<ToolSequence>> {
            Ok(vec![ToolSequence {
                tool_a: "Read".into(),
                tool_b: "Edit".into(),
                count: 5,
                avg_interval_secs: 2.0,
            }])
        }
        fn model_switching_patterns(&self) -> Result<Vec<ModelSwitch>> {
            Ok(vec![])
        }
        fn hourly_efficiency(&self) -> Result<Vec<HourlyEfficiency>> {
            Ok(vec![])
        }
    }

    #[test]
    fn query_returns_tool_sequences() {
        let uc = AnalyticsUseCase::new(Arc::new(MockAnalytics));
        let resp = uc.query().unwrap();
        assert_eq!(resp.tool_sequences.len(), 1);
        assert_eq!(resp.tool_sequences[0].tool_a, "Read");
    }
}
