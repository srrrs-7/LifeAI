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

    let _ = writeln!(out, "# HELP cc_project_sessions_total Sessions per project");
    let _ = writeln!(out, "# TYPE cc_project_sessions_total gauge");
    let _ = writeln!(
        out,
        "# HELP cc_project_tokens_total Total tokens per project"
    );
    let _ = writeln!(out, "# TYPE cc_project_tokens_total gauge");
    let _ = writeln!(out, "# HELP cc_project_cost_usd Cost in USD per project");
    let _ = writeln!(out, "# TYPE cc_project_cost_usd gauge");
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
            &[("project", &p.project)],
            p.total_tokens,
        );
        let _ = writeln!(
            out,
            "cc_project_cost_usd{{project=\"{}\"}} {:.6}",
            p.project, p.cost_usd
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
    use crate::domain::model::{MetricsSummary, ProjectSummary};

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
                total_tokens: 1000,
                cost_usd: 0.001234,
            }],
            ..Default::default()
        };
        let out = render(&s);
        assert!(out.contains("cc_project_cost_usd{project=\"my-proj\"} 0.001234"));
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
}
