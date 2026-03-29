use anyhow::Result;
use std::sync::Arc;

use crate::domain::{
    cost,
    model::{CostOptimizationSuggestion, OptimizationReport, SessionCostProfile},
    port::OptimizationPort,
};

/// ツールコール数がこの値以下なら「短いセッション」と判定
const SHORT_SESSION_TOOL_CALLS: i64 = 5;
/// 総トークン数がこの値以下なら「軽量セッション」と判定
const LOW_TOKEN_THRESHOLD: i64 = 10_000;

pub struct CostOptimizationUseCase {
    port: Arc<dyn OptimizationPort>,
}

impl CostOptimizationUseCase {
    pub fn new(port: Arc<dyn OptimizationPort>) -> Self {
        Self { port }
    }

    pub fn analyze(&self, period_days: Option<u32>) -> Result<OptimizationReport> {
        let profiles = self.port.find_overprovisioned_sessions(period_days)?;
        let suggestions: Vec<CostOptimizationSuggestion> =
            profiles.iter().filter_map(|p| self.evaluate(p)).collect();
        let total_savings = suggestions.iter().map(|s| s.estimated_savings_usd).sum();
        Ok(OptimizationReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            suggestions,
            total_potential_savings_usd: total_savings,
        })
    }

    fn evaluate(&self, p: &SessionCostProfile) -> Option<CostOptimizationSuggestion> {
        if !cost::is_expensive_model(&p.model) {
            return None;
        }

        let total_tokens = p.input_tokens + p.output_tokens;
        let reason = if p.tool_calls <= SHORT_SESSION_TOOL_CALLS {
            format!("短セッション（ツール{}回）で Opus を使用", p.tool_calls)
        } else if total_tokens <= LOW_TOKEN_THRESHOLD {
            format!("軽量セッション（{}トークン）で Opus を使用", total_tokens)
        } else {
            return None;
        };

        let sonnet_cost = cost::calculate_sonnet_alternative(
            p.input_tokens,
            p.output_tokens,
            p.cache_creation_tokens,
            p.cache_read_tokens,
        );
        let savings = p.cost_usd - sonnet_cost;
        if savings <= 0.0 {
            return None;
        }

        Some(CostOptimizationSuggestion {
            session_id: p.session_id.clone(),
            model_used: p.model.clone(),
            suggested_model: "claude-sonnet-4-6".to_string(),
            actual_cost_usd: p.cost_usd,
            estimated_savings_usd: savings,
            reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockOptPort(Vec<SessionCostProfile>);
    impl OptimizationPort for MockOptPort {
        fn find_overprovisioned_sessions(
            &self,
            _period_days: Option<u32>,
        ) -> Result<Vec<SessionCostProfile>> {
            Ok(self.0.clone())
        }
    }

    fn opus_profile(
        session_id: &str,
        tool_calls: i64,
        tokens: i64,
        cost: f64,
    ) -> SessionCostProfile {
        SessionCostProfile {
            session_id: session_id.to_string(),
            model: "claude-opus-4-6".to_string(),
            cost_usd: cost,
            input_tokens: tokens / 2,
            output_tokens: tokens / 2,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            tool_calls,
        }
    }

    #[test]
    fn short_opus_session_gets_suggestion() {
        let port = Arc::new(MockOptPort(vec![opus_profile("s1", 3, 8000, 5.0)]));
        let uc = CostOptimizationUseCase::new(port);
        let report = uc.analyze(None).unwrap();
        assert_eq!(report.suggestions.len(), 1);
        assert!(report.suggestions[0].estimated_savings_usd > 0.0);
        assert!(report.suggestions[0].reason.contains("短セッション"));
    }

    #[test]
    fn low_token_opus_session_gets_suggestion() {
        let port = Arc::new(MockOptPort(vec![opus_profile("s1", 20, 8000, 5.0)]));
        let uc = CostOptimizationUseCase::new(port);
        let report = uc.analyze(None).unwrap();
        assert_eq!(report.suggestions.len(), 1);
        assert!(report.suggestions[0].reason.contains("軽量セッション"));
    }

    #[test]
    fn heavy_opus_session_no_suggestion() {
        let port = Arc::new(MockOptPort(vec![opus_profile("s1", 50, 100_000, 50.0)]));
        let uc = CostOptimizationUseCase::new(port);
        let report = uc.analyze(None).unwrap();
        assert!(report.suggestions.is_empty());
    }

    #[test]
    fn empty_sessions_empty_report() {
        let port = Arc::new(MockOptPort(vec![]));
        let uc = CostOptimizationUseCase::new(port);
        let report = uc.analyze(None).unwrap();
        assert!(report.suggestions.is_empty());
        assert!((report.total_potential_savings_usd - 0.0).abs() < 1e-9);
    }
}
