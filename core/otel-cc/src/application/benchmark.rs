use anyhow::Result;
use std::sync::Arc;

use crate::domain::{
    model::{BestPractice, TeamBenchmark, UserBenchmark},
    port::BenchmarkPort,
};

pub struct BenchmarkUseCase {
    port: Arc<dyn BenchmarkPort>,
}

impl BenchmarkUseCase {
    pub fn new(port: Arc<dyn BenchmarkPort>) -> Self {
        Self { port }
    }

    pub fn analyze(&self, period_days: Option<u32>) -> Result<TeamBenchmark> {
        let mut benchmarks = self.port.user_efficiency_metrics(period_days)?;

        // 効率スコアでランク付け（低コスト + 高キャッシュ率 + 低エラー率 = 高効率）
        benchmarks.sort_by(|a, b| {
            let score_a = Self::efficiency_score(a);
            let score_b = Self::efficiency_score(b);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, b) in benchmarks.iter_mut().enumerate() {
            b.rank = (i + 1) as u32;
        }

        let best_practices = Self::extract_best_practices(&benchmarks);

        Ok(TeamBenchmark {
            generated_at: chrono::Utc::now().to_rfc3339(),
            user_benchmarks: benchmarks,
            best_practices,
        })
    }

    /// 効率スコア（高いほど良い）
    fn efficiency_score(b: &UserBenchmark) -> f64 {
        // キャッシュ率を重視し、コストとエラー率をペナルティとして差し引く
        b.cache_hit_ratio * 100.0 - b.cost_per_session - b.tool_error_rate * 100.0
    }

    fn extract_best_practices(benchmarks: &[UserBenchmark]) -> Vec<BestPractice> {
        if benchmarks.is_empty() {
            return vec![];
        }
        let mut practices = Vec::new();

        // 最高キャッシュヒット率
        if let Some(best) = benchmarks.iter().filter(|b| b.sessions > 0).max_by(|a, b| {
            a.cache_hit_ratio
                .partial_cmp(&b.cache_hit_ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            practices.push(BestPractice {
                user: best.user.clone(),
                metric: "cache_hit_ratio".to_string(),
                value: best.cache_hit_ratio,
                description: format!(
                    "最高キャッシュヒット率 {:.1}%",
                    best.cache_hit_ratio * 100.0
                ),
            });
        }

        // 最低コスト/セッション（セッション数 > 0）
        if let Some(best) = benchmarks.iter().filter(|b| b.sessions > 0).min_by(|a, b| {
            a.cost_per_session
                .partial_cmp(&b.cost_per_session)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            practices.push(BestPractice {
                user: best.user.clone(),
                metric: "cost_per_session".to_string(),
                value: best.cost_per_session,
                description: format!("最低セッション単価 ${:.2}", best.cost_per_session),
            });
        }

        // 最低ツールエラー率（ツール使用 > 0）
        if let Some(best) = benchmarks.iter().filter(|b| b.sessions > 0).min_by(|a, b| {
            a.tool_error_rate
                .partial_cmp(&b.tool_error_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            practices.push(BestPractice {
                user: best.user.clone(),
                metric: "tool_error_rate".to_string(),
                value: best.tool_error_rate,
                description: format!("最低ツールエラー率 {:.1}%", best.tool_error_rate * 100.0),
            });
        }

        practices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBenchmarkPort(Vec<UserBenchmark>);
    impl BenchmarkPort for MockBenchmarkPort {
        fn user_efficiency_metrics(&self, _period_days: Option<u32>) -> Result<Vec<UserBenchmark>> {
            Ok(self.0.clone())
        }
    }

    fn user(name: &str, cost: f64, cache: f64, error: f64) -> UserBenchmark {
        UserBenchmark {
            user: name.to_string(),
            sessions: 10,
            cost_per_session: cost,
            cache_hit_ratio: cache,
            tool_error_rate: error,
            total_cost_usd: cost * 10.0,
            rank: 0,
        }
    }

    #[test]
    fn ranks_by_efficiency() {
        let port = Arc::new(MockBenchmarkPort(vec![
            user("alice", 5.0, 0.95, 0.02), // best
            user("bob", 12.0, 0.80, 0.08),  // worst
        ]));
        let uc = BenchmarkUseCase::new(port);
        let report = uc.analyze(None).unwrap();
        assert_eq!(report.user_benchmarks[0].user, "alice");
        assert_eq!(report.user_benchmarks[0].rank, 1);
        assert_eq!(report.user_benchmarks[1].user, "bob");
        assert_eq!(report.user_benchmarks[1].rank, 2);
    }

    #[test]
    fn best_practices_extracted() {
        let port = Arc::new(MockBenchmarkPort(vec![
            user("alice", 5.0, 0.95, 0.02),
            user("bob", 3.0, 0.80, 0.01),
        ]));
        let uc = BenchmarkUseCase::new(port);
        let report = uc.analyze(None).unwrap();
        assert_eq!(report.best_practices.len(), 3);
        // cache_hit_ratio best = alice (0.95)
        let cache_bp = report
            .best_practices
            .iter()
            .find(|p| p.metric == "cache_hit_ratio")
            .unwrap();
        assert_eq!(cache_bp.user, "alice");
        // cost best = bob ($3)
        let cost_bp = report
            .best_practices
            .iter()
            .find(|p| p.metric == "cost_per_session")
            .unwrap();
        assert_eq!(cost_bp.user, "bob");
    }

    #[test]
    fn empty_benchmarks_returns_empty() {
        let port = Arc::new(MockBenchmarkPort(vec![]));
        let uc = BenchmarkUseCase::new(port);
        let report = uc.analyze(None).unwrap();
        assert!(report.user_benchmarks.is_empty());
        assert!(report.best_practices.is_empty());
    }
}
