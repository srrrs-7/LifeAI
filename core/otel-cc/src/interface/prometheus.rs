use crate::domain::model::MetricsSummary;
use std::fmt::Write;

/// MetricsSummary を Prometheus テキスト形式に変換する
pub fn render(s: &MetricsSummary) -> String {
    let mut out = String::new();

    metric_gauge(
        &mut out,
        "cc_sessions_total",
        "Total Claude Code sessions",
        &[],
        s.total_sessions as f64,
    );
    metric_gauge(
        &mut out,
        "cc_active_sessions",
        "Active sessions",
        &[],
        s.active_sessions as f64,
    );

    let _ = writeln!(out, "# HELP cc_tokens_total Total tokens by type");
    let _ = writeln!(out, "# TYPE cc_tokens_total counter");
    labeled(
        &mut out,
        "cc_tokens_total",
        &[("type", "input")],
        s.total_input_tokens,
    );
    labeled(
        &mut out,
        "cc_tokens_total",
        &[("type", "output")],
        s.total_output_tokens,
    );
    labeled(
        &mut out,
        "cc_tokens_total",
        &[("type", "cache_create")],
        s.total_cache_creation_tokens,
    );
    labeled(
        &mut out,
        "cc_tokens_total",
        &[("type", "cache_read")],
        s.total_cache_read_tokens,
    );

    let total_with_cache = s.total_input_tokens + s.total_cache_read_tokens;
    let cache_hit_ratio = if total_with_cache > 0 {
        s.total_cache_read_tokens as f64 / total_with_cache as f64
    } else {
        0.0
    };
    metric_gauge(
        &mut out,
        "cc_cache_hit_ratio",
        "Cache read ratio (cache_read / total_input)",
        &[],
        cache_hit_ratio,
    );
    metric_float(
        &mut out,
        "cc_cost_usd_total",
        "Total cost in USD (counter)",
        s.total_cost_usd,
    );

    let _ = writeln!(out, "# HELP cc_tool_calls_total Tool call counts by name");
    let _ = writeln!(out, "# TYPE cc_tool_calls_total counter");
    let _ = writeln!(out, "# HELP cc_tool_errors_total Tool error counts by name");
    let _ = writeln!(out, "# TYPE cc_tool_errors_total counter");
    for (tool, count, errors) in &s.tool_counts {
        labeled(&mut out, "cc_tool_calls_total", &[("tool", tool)], *count);
        labeled(&mut out, "cc_tool_errors_total", &[("tool", tool)], *errors);
    }

    let error_rate = if s.total_tool_calls > 0 {
        s.total_tool_errors as f64 / s.total_tool_calls as f64
    } else {
        0.0
    };
    metric_gauge(
        &mut out,
        "cc_tool_error_rate",
        "Overall tool error rate",
        &[],
        error_rate,
    );

    metric_float(
        &mut out,
        "cc_compression_events_total",
        "Total context compression events detected (counter)",
        s.total_compression_events as f64,
    );

    let _ = writeln!(
        out,
        "# HELP cc_entrypoint_sessions_total Sessions per entrypoint (cli or slash command)"
    );
    let _ = writeln!(out, "# TYPE cc_entrypoint_sessions_total counter");
    for (ep, count) in &s.entrypoint_counts {
        labeled(
            &mut out,
            "cc_entrypoint_sessions_total",
            &[("entrypoint", ep)],
            *count,
        );
    }

    let _ = writeln!(out, "# HELP cc_project_sessions_total Sessions per project");
    let _ = writeln!(out, "# TYPE cc_project_sessions_total gauge");
    let _ = writeln!(
        out,
        "# HELP cc_project_tokens_total Tokens per project by type"
    );
    let _ = writeln!(out, "# TYPE cc_project_tokens_total gauge");
    let _ = writeln!(out, "# HELP cc_project_cost_usd Cost in USD per project");
    let _ = writeln!(out, "# TYPE cc_project_cost_usd gauge");
    let _ = writeln!(
        out,
        "# HELP cc_project_tool_calls_total Tool calls per project"
    );
    let _ = writeln!(out, "# TYPE cc_project_tool_calls_total gauge");
    for p in &s.projects {
        labeled(
            &mut out,
            "cc_project_sessions_total",
            &[("project", &p.project)],
            p.sessions,
        );
        labeled(
            &mut out,
            "cc_project_tokens_total",
            &[("project", &p.project), ("type", "input")],
            p.input_tokens,
        );
        labeled(
            &mut out,
            "cc_project_tokens_total",
            &[("project", &p.project), ("type", "output")],
            p.output_tokens,
        );
        labeled(
            &mut out,
            "cc_project_tokens_total",
            &[("project", &p.project), ("type", "cache_create")],
            p.cache_creation_tokens,
        );
        labeled(
            &mut out,
            "cc_project_tokens_total",
            &[("project", &p.project), ("type", "cache_read")],
            p.cache_read_tokens,
        );
        let _ = writeln!(
            out,
            "cc_project_cost_usd{{project=\"{}\"}} {:.6}",
            p.project, p.cost_usd
        );
        labeled(
            &mut out,
            "cc_project_tool_calls_total",
            &[("project", &p.project)],
            p.tool_calls,
        );
    }

    // ── モデル別メトリクス ─────────────────────────────────────────
    let _ = writeln!(out, "# HELP cc_model_sessions Sessions per model");
    let _ = writeln!(out, "# TYPE cc_model_sessions gauge");
    let _ = writeln!(out, "# HELP cc_model_tokens_total Tokens per model by type");
    let _ = writeln!(out, "# TYPE cc_model_tokens_total gauge");
    let _ = writeln!(out, "# HELP cc_model_cost_usd Cost in USD per model");
    let _ = writeln!(out, "# TYPE cc_model_cost_usd gauge");
    for m in &s.model_counts {
        labeled(
            &mut out,
            "cc_model_sessions",
            &[("model", &m.model)],
            m.sessions,
        );
        labeled(
            &mut out,
            "cc_model_tokens_total",
            &[("model", &m.model), ("type", "input")],
            m.input_tokens,
        );
        labeled(
            &mut out,
            "cc_model_tokens_total",
            &[("model", &m.model), ("type", "output")],
            m.output_tokens,
        );
        labeled(
            &mut out,
            "cc_model_tokens_total",
            &[("model", &m.model), ("type", "cache_create")],
            m.cache_creation_tokens,
        );
        labeled(
            &mut out,
            "cc_model_tokens_total",
            &[("model", &m.model), ("type", "cache_read")],
            m.cache_read_tokens,
        );
        let _ = writeln!(
            out,
            "cc_model_cost_usd{{model=\"{}\"}} {:.6}",
            m.model, m.cost_usd
        );
    }

    // ── ユーザー別メトリクス ────────────────────────────────────
    let _ = writeln!(out, "# HELP cc_user_sessions Sessions per user");
    let _ = writeln!(out, "# TYPE cc_user_sessions gauge");
    let _ = writeln!(out, "# HELP cc_user_tokens_total Tokens per user by type");
    let _ = writeln!(out, "# TYPE cc_user_tokens_total gauge");
    let _ = writeln!(out, "# HELP cc_user_cost_usd Cost in USD per user");
    let _ = writeln!(out, "# TYPE cc_user_cost_usd gauge");
    let _ = writeln!(out, "# HELP cc_user_tool_calls_total Tool calls per user");
    let _ = writeln!(out, "# TYPE cc_user_tool_calls_total gauge");
    for u in &s.user_counts {
        labeled(
            &mut out,
            "cc_user_sessions",
            &[("user", &u.user)],
            u.sessions,
        );
        labeled(
            &mut out,
            "cc_user_tokens_total",
            &[("user", &u.user), ("type", "input")],
            u.input_tokens,
        );
        labeled(
            &mut out,
            "cc_user_tokens_total",
            &[("user", &u.user), ("type", "output")],
            u.output_tokens,
        );
        let _ = writeln!(
            out,
            "cc_user_cost_usd{{user=\"{}\"}} {:.6}",
            u.user, u.cost_usd
        );
        labeled(
            &mut out,
            "cc_user_tool_calls_total",
            &[("user", &u.user)],
            u.tool_calls,
        );
    }

    out
}

fn metric_gauge(out: &mut String, name: &str, help: &str, labels: &[(&str, &str)], val: f64) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} gauge");
    if labels.is_empty() {
        let _ = writeln!(out, "{name} {val:.6}");
    } else {
        let ls = label_str(labels);
        let _ = writeln!(out, "{name}{{{ls}}} {val:.6}");
    }
}

fn metric_float(out: &mut String, name: &str, help: &str, val: f64) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} counter");
    let _ = writeln!(out, "{name} {val:.6}");
}

fn labeled(out: &mut String, name: &str, labels: &[(&str, &str)], val: i64) {
    let ls = label_str(labels);
    let _ = writeln!(out, "{name}{{{ls}}} {val}");
}

fn label_str(labels: &[(&str, &str)]) -> String {
    labels
        .iter()
        .map(|(k, v)| format!("{k}=\"{v}\""))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{MetricsSummary, ModelSummary, ProjectSummary, UserSummary};

    fn render_default() -> String {
        render(&MetricsSummary::default())
    }

    // ── HELP / TYPE 行の存在確認 ───────────────────────────────

    #[test]
    fn help_and_gauge_type_for_sessions() {
        let out = render_default();
        assert!(out.contains("# HELP cc_sessions_total Total Claude Code sessions"));
        assert!(out.contains("# TYPE cc_sessions_total gauge"));
    }

    #[test]
    fn compression_events_total_uses_counter_type() {
        // _total サフィックスのメトリクスは TYPE counter でなければならない
        let out = render_default();
        let lines: Vec<&str> = out.lines().collect();
        let type_line = lines
            .iter()
            .find(|l| l.contains("TYPE") && l.contains("cc_compression_events_total"))
            .expect("TYPE line for cc_compression_events_total not found");
        assert!(
            type_line.contains("counter"),
            "TYPE must be 'counter', got: {type_line}"
        );
    }

    #[test]
    fn tokens_total_uses_counter_type() {
        let out = render_default();
        let lines: Vec<&str> = out.lines().collect();
        let type_line = lines
            .iter()
            .find(|l| l.contains("TYPE") && l.contains("cc_tokens_total"))
            .unwrap();
        assert!(type_line.contains("counter"));
    }

    // ── 値の正確性 ────────────────────────────────────────────

    #[test]
    fn tool_counts_rendered_per_tool_with_label() {
        let s = MetricsSummary {
            tool_counts: vec![("Bash".to_string(), 5, 2), ("Read".to_string(), 10, 0)],
            ..Default::default()
        };
        let out = render(&s);
        assert!(out.contains("cc_tool_calls_total{tool=\"Bash\"} 5"));
        assert!(out.contains("cc_tool_errors_total{tool=\"Bash\"} 2"));
        assert!(out.contains("cc_tool_calls_total{tool=\"Read\"} 10"));
        assert!(out.contains("cc_tool_errors_total{tool=\"Read\"} 0"));
    }

    #[test]
    fn project_cost_rendered_with_six_decimal_places() {
        let s = MetricsSummary {
            projects: vec![ProjectSummary {
                project: "my-proj".to_string(),
                sessions: 3,
                input_tokens: 800,
                output_tokens: 200,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.001234,
                tool_calls: 0,
            }],
            ..Default::default()
        };
        let out = render(&s);
        assert!(out.contains("cc_project_cost_usd{project=\"my-proj\"} 0.001234"));
        assert!(out.contains("cc_project_tokens_total{project=\"my-proj\",type=\"input\"} 800"));
        assert!(out.contains("cc_project_tokens_total{project=\"my-proj\",type=\"output\"} 200"));
    }

    #[test]
    fn cache_hit_ratio_is_zero_when_no_tokens() {
        let out = render_default();
        assert!(out.contains("cc_cache_hit_ratio 0.000000"));
    }

    #[test]
    fn tool_error_rate_is_zero_when_no_calls() {
        let out = render_default();
        assert!(out.contains("cc_tool_error_rate 0.000000"));
    }

    // ── エントリーポイント ─────────────────────────────────────

    #[test]
    fn entrypoint_sessions_total_uses_counter_type() {
        let out = render_default();
        let lines: Vec<&str> = out.lines().collect();
        let type_line = lines
            .iter()
            .find(|l| l.contains("TYPE") && l.contains("cc_entrypoint_sessions_total"))
            .expect("TYPE line for cc_entrypoint_sessions_total not found");
        assert!(
            type_line.contains("counter"),
            "expected TYPE counter, got: {type_line}"
        );
    }

    #[test]
    fn entrypoint_sessions_total_rendered_with_label() {
        let s = MetricsSummary {
            entrypoint_counts: vec![("cli".to_string(), 10), ("daily-report".to_string(), 3)],
            ..Default::default()
        };
        let out = render(&s);
        assert!(out.contains("cc_entrypoint_sessions_total{entrypoint=\"cli\"} 10"));
        assert!(out.contains("cc_entrypoint_sessions_total{entrypoint=\"daily-report\"} 3"));
    }

    #[test]
    fn entrypoint_sessions_total_not_rendered_when_no_data() {
        let out = render_default();
        // HELP/TYPE 行は出力されるが、データ行（ラベル付き）はゼロ件
        assert!(out.contains("# HELP cc_entrypoint_sessions_total"));
        assert!(
            !out.contains("cc_entrypoint_sessions_total{"),
            "no data lines expected when entrypoint_counts is empty"
        );
    }

    // ── モデル別メトリクス ────────────────────────────────────────

    #[test]
    fn model_metrics_rendered_with_labels() {
        let s = MetricsSummary {
            model_counts: vec![
                ModelSummary {
                    model: "claude-sonnet-4-20250514".to_string(),
                    sessions: 5,
                    input_tokens: 1000,
                    output_tokens: 500,
                    cache_creation_tokens: 200,
                    cache_read_tokens: 800,
                    cost_usd: 0.012345,
                },
                ModelSummary {
                    model: "claude-opus-4-20250514".to_string(),
                    sessions: 2,
                    input_tokens: 3000,
                    output_tokens: 1500,
                    cache_creation_tokens: 600,
                    cache_read_tokens: 2400,
                    cost_usd: 0.567890,
                },
            ],
            ..Default::default()
        };
        let out = render(&s);
        // sessions
        assert!(out.contains("cc_model_sessions{model=\"claude-sonnet-4-20250514\"} 5"));
        assert!(out.contains("cc_model_sessions{model=\"claude-opus-4-20250514\"} 2"));
        // tokens
        assert!(out.contains(
            "cc_model_tokens_total{model=\"claude-sonnet-4-20250514\",type=\"input\"} 1000"
        ));
        assert!(out.contains(
            "cc_model_tokens_total{model=\"claude-opus-4-20250514\",type=\"output\"} 1500"
        ));
        // cost
        assert!(out.contains("cc_model_cost_usd{model=\"claude-sonnet-4-20250514\"} 0.012345"));
        assert!(out.contains("cc_model_cost_usd{model=\"claude-opus-4-20250514\"} 0.567890"));
    }

    #[test]
    fn model_metrics_not_rendered_when_empty() {
        let out = render_default();
        assert!(out.contains("# HELP cc_model_sessions"));
        assert!(
            !out.contains("cc_model_sessions{"),
            "no data lines expected when model_counts is empty"
        );
    }

    // ── ユーザー別メトリクス ─────────────────────────────────────

    #[test]
    fn user_metrics_rendered_with_labels() {
        let s = MetricsSummary {
            user_counts: vec![
                UserSummary {
                    user: "alice".to_string(),
                    sessions: 5,
                    input_tokens: 1000,
                    output_tokens: 500,
                    cost_usd: 0.123456,
                    tool_calls: 10,
                },
                UserSummary {
                    user: "bob".to_string(),
                    sessions: 3,
                    input_tokens: 2000,
                    output_tokens: 800,
                    cost_usd: 0.654321,
                    tool_calls: 7,
                },
            ],
            ..Default::default()
        };
        let out = render(&s);
        assert!(out.contains("cc_user_sessions{user=\"alice\"} 5"));
        assert!(out.contains("cc_user_sessions{user=\"bob\"} 3"));
        assert!(out.contains("cc_user_tokens_total{user=\"alice\",type=\"input\"} 1000"));
        assert!(out.contains("cc_user_tokens_total{user=\"bob\",type=\"output\"} 800"));
        assert!(out.contains("cc_user_cost_usd{user=\"alice\"} 0.123456"));
        assert!(out.contains("cc_user_tool_calls_total{user=\"bob\"} 7"));
    }

    #[test]
    fn user_metrics_not_rendered_when_empty() {
        let out = render_default();
        assert!(out.contains("# HELP cc_user_sessions"));
        assert!(
            !out.contains("cc_user_sessions{"),
            "no data lines expected when user_counts is empty"
        );
    }
}
