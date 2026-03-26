use serde::Deserialize;

// ── OTLP/HTTP JSON 型定義（最小限） ────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TracesPayload {
    #[serde(rename = "resourceSpans", default)]
    pub resource_spans: Vec<ResourceSpans>,
}

#[derive(Debug, Deserialize)]
pub struct ResourceSpans {
    #[serde(rename = "scopeSpans", default)]
    pub scope_spans: Vec<ScopeSpans>,
}

#[derive(Debug, Deserialize)]
pub struct ScopeSpans {
    #[serde(default)]
    pub spans: Vec<Span>,
}

#[derive(Debug, Deserialize)]
pub struct Span {
    #[serde(rename = "traceId")]
    pub trace_id: Option<String>,
    #[serde(rename = "spanId")]
    pub span_id: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub attributes: Vec<KeyValue>,
}

#[derive(Debug, Deserialize)]
pub struct MetricsPayload {
    #[serde(rename = "resourceMetrics", default)]
    pub resource_metrics: Vec<ResourceMetrics>,
}

#[derive(Debug, Deserialize)]
pub struct ResourceMetrics {
    #[serde(rename = "scopeMetrics", default)]
    pub scope_metrics: Vec<ScopeMetrics>,
}

#[derive(Debug, Deserialize)]
pub struct ScopeMetrics {
    #[serde(default)]
    pub metrics: Vec<Metric>,
}

#[derive(Debug, Deserialize)]
pub struct Metric {
    pub name: Option<String>,
    pub sum: Option<DataPoints>,
    pub gauge: Option<DataPoints>,
}

#[derive(Debug, Deserialize)]
pub struct DataPoints {
    #[serde(rename = "dataPoints", default)]
    pub data_points: Vec<NumberDataPoint>,
}

#[derive(Debug, Deserialize)]
pub struct NumberDataPoint {
    #[serde(rename = "asInt")]
    pub as_int: Option<i64>,
    #[allow(dead_code)]
    #[serde(rename = "asDouble")]
    pub as_double: Option<f64>,
    #[allow(dead_code)]
    #[serde(default)]
    pub attributes: Vec<KeyValue>,
}

#[derive(Debug, Deserialize)]
pub struct KeyValue {
    pub key: Option<String>,
    pub value: Option<AnyValue>,
}

#[derive(Debug, Deserialize)]
pub struct AnyValue {
    #[serde(rename = "stringValue")]
    pub string_value: Option<String>,
    #[serde(rename = "intValue")]
    pub int_value: Option<i64>,
    #[allow(dead_code)]
    #[serde(rename = "doubleValue")]
    pub double_value: Option<f64>,
}

// ── パース済みイベント型 ────────────────────────────────────────────

pub struct ExtractedTokenEvent {
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub span_name: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub model: Option<String>,
    pub session_id: Option<String>,
    /// claude_code.project または service.name から取得
    pub project: Option<String>,
}

pub struct ExtractedMetric {
    pub name: String,
    #[allow(dead_code)]
    pub value_int: Option<i64>,
}

// ── パース関数 ─────────────────────────────────────────────────────

/// TracesPayload からトークン情報を持つスパンを抽出する
pub fn extract_token_events(payload: &TracesPayload) -> Vec<ExtractedTokenEvent> {
    let mut events = Vec::new();

    for rs in &payload.resource_spans {
        for ss in &rs.scope_spans {
            for span in &ss.spans {
                let attrs = span_attrs(span);

                let has_cc = attrs
                    .keys()
                    .any(|k| k.starts_with("claude_code") || k.starts_with("llm.usage"));
                if !has_cc {
                    continue;
                }

                events.push(ExtractedTokenEvent {
                    trace_id: span.trace_id.clone(),
                    span_id: span.span_id.clone(),
                    span_name: span.name.clone(),
                    input_tokens: attr_i64(
                        &attrs,
                        &["llm.usage.prompt_tokens", "claude_code.token.input"],
                    ),
                    output_tokens: attr_i64(
                        &attrs,
                        &["llm.usage.completion_tokens", "claude_code.token.output"],
                    ),
                    cache_creation_tokens: attr_i64(&attrs, &["claude_code.token.cache_creation"]),
                    cache_read_tokens: attr_i64(&attrs, &["claude_code.token.cache_read"]),
                    model: attrs
                        .get("llm.model")
                        .or_else(|| attrs.get("claude_code.model"))
                        .cloned(),
                    session_id: attrs.get("claude_code.session_id").cloned(),
                    project: attrs
                        .get("claude_code.project")
                        .or_else(|| attrs.get("service.name"))
                        .cloned(),
                });
            }
        }
    }

    events
}

/// MetricsPayload からメトリクス名と値を抽出する
pub fn extract_metrics(payload: &MetricsPayload) -> Vec<ExtractedMetric> {
    let mut result = Vec::new();

    for rm in &payload.resource_metrics {
        for sm in &rm.scope_metrics {
            for metric in &sm.metrics {
                let Some(name) = &metric.name else { continue };
                let points = metric
                    .sum
                    .as_ref()
                    .map(|s| &s.data_points)
                    .or_else(|| metric.gauge.as_ref().map(|g| &g.data_points));
                if let Some(pts) = points {
                    for dp in pts {
                        result.push(ExtractedMetric {
                            name: name.clone(),
                            value_int: dp.as_int,
                        });
                    }
                }
            }
        }
    }

    result
}

fn span_attrs(span: &Span) -> std::collections::HashMap<String, String> {
    span.attributes
        .iter()
        .filter_map(|kv| {
            let k = kv.key.clone()?;
            let v = kv.value.as_ref().and_then(|v| {
                v.string_value
                    .clone()
                    .or_else(|| v.int_value.map(|i| i.to_string()))
            })?;
            Some((k, v))
        })
        .collect()
}

fn attr_i64(attrs: &std::collections::HashMap<String, String>, keys: &[&str]) -> i64 {
    for key in keys {
        if let Some(v) = attrs.get(*key).and_then(|s| s.parse().ok()) {
            return v;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traces_json(attrs: &str) -> String {
        format!(
            r#"{{"resourceSpans":[{{"scopeSpans":[{{"spans":[{{"traceId":"tid","spanId":"sid","name":"cc.req","attributes":[{attrs}]}}]}}]}}]}}"#
        )
    }

    // ── extract_token_events ────────────────────────────────────

    #[test]
    fn extracts_token_counts_from_llm_usage_attrs() {
        let json = traces_json(
            r#"{"key":"llm.usage.prompt_tokens","value":{"intValue":100}},
               {"key":"llm.usage.completion_tokens","value":{"intValue":50}},
               {"key":"claude_code.session_id","value":{"stringValue":"sess-1"}},
               {"key":"claude_code.project","value":{"stringValue":"my-proj"}}"#,
        );
        let payload: TracesPayload = serde_json::from_str(&json).unwrap();
        let events = extract_token_events(&payload);

        assert_eq!(events.len(), 1);
        let ev = &events[0];
        assert_eq!(ev.input_tokens, 100);
        assert_eq!(ev.output_tokens, 50);
        assert_eq!(ev.session_id.as_deref(), Some("sess-1"));
        assert_eq!(ev.project.as_deref(), Some("my-proj"));
        assert_eq!(ev.trace_id.as_deref(), Some("tid"));
        assert_eq!(ev.span_id.as_deref(), Some("sid"));
    }

    #[test]
    fn skips_spans_without_claude_or_llm_attrs() {
        let json = traces_json(r#"{"key":"http.method","value":{"stringValue":"GET"}}"#);
        let payload: TracesPayload = serde_json::from_str(&json).unwrap();
        let events = extract_token_events(&payload);
        assert!(events.is_empty());
    }

    #[test]
    fn project_falls_back_to_service_name() {
        let json = traces_json(
            r#"{"key":"claude_code.token.input","value":{"intValue":10}},
               {"key":"service.name","value":{"stringValue":"fallback-proj"}}"#,
        );
        let payload: TracesPayload = serde_json::from_str(&json).unwrap();
        let events = extract_token_events(&payload);
        assert_eq!(events[0].project.as_deref(), Some("fallback-proj"));
    }

    #[test]
    fn empty_payload_returns_empty_vec() {
        let payload: TracesPayload = serde_json::from_str(r#"{"resourceSpans":[]}"#).unwrap();
        assert!(extract_token_events(&payload).is_empty());
    }

    // ── extract_metrics ─────────────────────────────────────────

    #[test]
    fn extracts_sum_data_points() {
        let json = r#"{
            "resourceMetrics":[{"scopeMetrics":[{"metrics":[{
                "name":"cc.tokens",
                "sum":{"dataPoints":[{"asInt":42}]}
            }]}]}]
        }"#;
        let payload: MetricsPayload = serde_json::from_str(json).unwrap();
        let metrics = extract_metrics(&payload);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "cc.tokens");
        assert_eq!(metrics[0].value_int, Some(42));
    }

    #[test]
    fn extracts_gauge_data_points() {
        let json = r#"{
            "resourceMetrics":[{"scopeMetrics":[{"metrics":[{
                "name":"cc.sessions",
                "gauge":{"dataPoints":[{"asInt":3}]}
            }]}]}]
        }"#;
        let payload: MetricsPayload = serde_json::from_str(json).unwrap();
        let metrics = extract_metrics(&payload);
        assert_eq!(metrics[0].name, "cc.sessions");
    }
}
