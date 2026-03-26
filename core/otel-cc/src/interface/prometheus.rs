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
