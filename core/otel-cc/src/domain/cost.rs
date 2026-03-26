/// モデル別単価 (USD per 1M tokens, 2026年3月時点)
struct Rates {
    input: f64,
    output: f64,
    cache_write: f64,
    cache_read: f64,
}

const OPUS: Rates = Rates { input: 15.0,   output: 75.0,  cache_write: 18.75, cache_read: 1.5  };
const SONNET: Rates = Rates { input: 3.0,  output: 15.0,  cache_write: 3.75,  cache_read: 0.3  };
const HAIKU: Rates = Rates { input: 0.25,  output: 1.25,  cache_write: 0.3,   cache_read: 0.03 };

fn rates_for(model: &str) -> &'static Rates {
    if model.contains("opus")  { &OPUS   }
    else if model.contains("haiku") { &HAIKU  }
    else { &SONNET }
}

pub fn calculate(
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
    cache_creation_tokens: i64,
    cache_read_tokens: i64,
) -> f64 {
    let r = rates_for(model);
    (input_tokens as f64 * r.input
        + output_tokens as f64 * r.output
        + cache_creation_tokens as f64 * r.cache_write
        + cache_read_tokens as f64 * r.cache_read)
        / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sonnet_1m_input_costs_3_usd() {
        let cost = calculate("claude-sonnet-4-6", 1_000_000, 0, 0, 0);
        assert!((cost - 3.0).abs() < 1e-9, "expected $3.00, got ${cost}");
    }

    #[test]
    fn opus_1m_output_costs_75_usd() {
        let cost = calculate("claude-opus-4-6", 0, 1_000_000, 0, 0);
        assert!((cost - 75.0).abs() < 1e-9, "expected $75.00, got ${cost}");
    }

    #[test]
    fn haiku_cache_read_cheaper_than_input() {
        let input_cost = calculate("claude-haiku-4-5", 1_000_000, 0, 0, 0);
        let cache_cost = calculate("claude-haiku-4-5", 0, 0, 0, 1_000_000);
        assert!(cache_cost < input_cost, "cache_read should be cheaper than input");
    }

    #[test]
    fn unknown_model_falls_back_to_sonnet() {
        let cost = calculate("claude-unknown-x", 1_000_000, 0, 0, 0);
        let sonnet_cost = calculate("claude-sonnet-4-6", 1_000_000, 0, 0, 0);
        assert!((cost - sonnet_cost).abs() < 1e-9);
    }

    #[test]
    fn zero_tokens_zero_cost() {
        assert_eq!(calculate("claude-sonnet-4-6", 0, 0, 0, 0), 0.0);
    }
}
