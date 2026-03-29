/// モデル別単価 (USD per 1M tokens)
/// 出典: https://platform.claude.com/docs/en/about-claude/pricing
struct Rates {
    input: f64,
    output: f64,
    cache_write: f64, // 5-minute cache write (1.25x input)
    cache_read: f64,  // cache hit (0.1x input)
}

/// Opus 4.5 / 4.6: $5 input, $25 output
const OPUS: Rates = Rates {
    input: 5.0,
    output: 25.0,
    cache_write: 6.25,
    cache_read: 0.5,
};
/// Opus 4.0 / 4.1: $15 input, $75 output (旧世代)
const OPUS_LEGACY: Rates = Rates {
    input: 15.0,
    output: 75.0,
    cache_write: 18.75,
    cache_read: 1.5,
};
/// Sonnet 4 / 4.5 / 4.6: $3 input, $15 output
const SONNET: Rates = Rates {
    input: 3.0,
    output: 15.0,
    cache_write: 3.75,
    cache_read: 0.3,
};
/// Haiku 4.5: $1 input, $5 output
const HAIKU: Rates = Rates {
    input: 1.0,
    output: 5.0,
    cache_write: 1.25,
    cache_read: 0.1,
};
/// Haiku 3.5: $0.80 input, $4 output
const HAIKU_35: Rates = Rates {
    input: 0.80,
    output: 4.0,
    cache_write: 1.0,
    cache_read: 0.08,
};
/// Haiku 3 (deprecated): $0.25 input, $1.25 output
const HAIKU_3: Rates = Rates {
    input: 0.25,
    output: 1.25,
    cache_write: 0.30,
    cache_read: 0.03,
};

fn rates_for(model: &str) -> &'static Rates {
    if model.contains("opus") {
        // Opus 4.5+ は新料金体系 ($5/$25)、それ以前は旧料金 ($15/$75)
        if model.contains("opus-4-5") || model.contains("opus-4-6") {
            &OPUS
        } else {
            &OPUS_LEGACY
        }
    } else if model.contains("haiku") {
        if model.contains("haiku-3-5") {
            &HAIKU_35
        } else if model.contains("haiku-3") || model.contains("haiku-20") {
            &HAIKU_3
        } else {
            &HAIKU
        }
    } else {
        &SONNET
    }
}

/// モデルが "高コスト" カテゴリか判定（Opus 系）
pub fn is_expensive_model(model: &str) -> bool {
    model.contains("opus")
}

/// 指定モデルの Sonnet 代替コストを計算
pub fn calculate_sonnet_alternative(
    input_tokens: i64,
    output_tokens: i64,
    cache_creation_tokens: i64,
    cache_read_tokens: i64,
) -> f64 {
    let r = &SONNET;
    (input_tokens as f64 * r.input
        + output_tokens as f64 * r.output
        + cache_creation_tokens as f64 * r.cache_write
        + cache_read_tokens as f64 * r.cache_read)
        / 1_000_000.0
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

    // ── Sonnet 4.6: $3 input, $15 output ─────────────────────────
    #[test]
    fn sonnet_1m_input_costs_3_usd() {
        let cost = calculate("claude-sonnet-4-6", 1_000_000, 0, 0, 0);
        assert!((cost - 3.0).abs() < 1e-9, "expected $3.00, got ${cost}");
    }

    #[test]
    fn sonnet_1m_output_costs_15_usd() {
        let cost = calculate("claude-sonnet-4-6", 0, 1_000_000, 0, 0);
        assert!((cost - 15.0).abs() < 1e-9, "expected $15.00, got ${cost}");
    }

    // ── Opus 4.6: $5 input, $25 output ───────────────────────────
    #[test]
    fn opus_46_1m_input_costs_5_usd() {
        let cost = calculate("claude-opus-4-6", 1_000_000, 0, 0, 0);
        assert!((cost - 5.0).abs() < 1e-9, "expected $5.00, got ${cost}");
    }

    #[test]
    fn opus_46_1m_output_costs_25_usd() {
        let cost = calculate("claude-opus-4-6", 0, 1_000_000, 0, 0);
        assert!((cost - 25.0).abs() < 1e-9, "expected $25.00, got ${cost}");
    }

    #[test]
    fn opus_46_cache_write_costs_6_25_usd() {
        let cost = calculate("claude-opus-4-6", 0, 0, 1_000_000, 0);
        assert!((cost - 6.25).abs() < 1e-9, "expected $6.25, got ${cost}");
    }

    #[test]
    fn opus_46_cache_read_costs_0_50_usd() {
        let cost = calculate("claude-opus-4-6", 0, 0, 0, 1_000_000);
        assert!((cost - 0.50).abs() < 1e-9, "expected $0.50, got ${cost}");
    }

    // ── Opus 4.0/4.1 (legacy): $15 input, $75 output ────────────
    #[test]
    fn opus_40_1m_input_costs_15_usd() {
        let cost = calculate("claude-opus-4-20250514", 1_000_000, 0, 0, 0);
        assert!((cost - 15.0).abs() < 1e-9, "expected $15.00, got ${cost}");
    }

    #[test]
    fn opus_41_1m_output_costs_75_usd() {
        let cost = calculate("claude-opus-4-1-20250805", 0, 1_000_000, 0, 0);
        assert!((cost - 75.0).abs() < 1e-9, "expected $75.00, got ${cost}");
    }

    // ── Haiku 4.5: $1 input, $5 output ───────────────────────────
    #[test]
    fn haiku_45_1m_input_costs_1_usd() {
        let cost = calculate("claude-haiku-4-5-20251001", 1_000_000, 0, 0, 0);
        assert!((cost - 1.0).abs() < 1e-9, "expected $1.00, got ${cost}");
    }

    #[test]
    fn haiku_45_1m_output_costs_5_usd() {
        let cost = calculate("claude-haiku-4-5-20251001", 0, 1_000_000, 0, 0);
        assert!((cost - 5.0).abs() < 1e-9, "expected $5.00, got ${cost}");
    }

    #[test]
    fn haiku_45_cache_read_cheaper_than_input() {
        let input_cost = calculate("claude-haiku-4-5-20251001", 1_000_000, 0, 0, 0);
        let cache_cost = calculate("claude-haiku-4-5-20251001", 0, 0, 0, 1_000_000);
        assert!(
            cache_cost < input_cost,
            "cache_read should be cheaper than input"
        );
    }

    // ── Haiku 3.5: $0.80 input, $4 output ────────────────────────
    #[test]
    fn haiku_35_1m_input_costs_0_80_usd() {
        let cost = calculate("claude-haiku-3-5-20241022", 1_000_000, 0, 0, 0);
        assert!((cost - 0.80).abs() < 1e-9, "expected $0.80, got ${cost}");
    }

    // ── Haiku 3 (deprecated): $0.25 input, $1.25 output ─────────
    #[test]
    fn haiku_3_1m_input_costs_0_25_usd() {
        let cost = calculate("claude-3-haiku-20240307", 1_000_000, 0, 0, 0);
        assert!((cost - 0.25).abs() < 1e-9, "expected $0.25, got ${cost}");
    }

    // ── フォールバック・境界値 ────────────────────────────────────
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

    #[test]
    fn opus_45_uses_new_pricing() {
        // Opus 4.5 は 4.6 と同じ新料金体系
        let cost = calculate("claude-opus-4-5-20251101", 1_000_000, 0, 0, 0);
        assert!((cost - 5.0).abs() < 1e-9, "expected $5.00, got ${cost}");
    }
}
