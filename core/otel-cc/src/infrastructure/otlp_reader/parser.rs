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

                let has_cc = attrs.keys().any(|k| {
                    k.starts_with("claude_code") || k.starts_with("llm.usage")
                });
                if !has_cc {
                    continue;
                }

                events.push(ExtractedTokenEvent {
                    trace_id: span.trace_id.clone(),
                    span_id: span.span_id.clone(),
                    span_name: span.name.clone(),
                    input_tokens: attr_i64(&attrs, &["llm.usage.prompt_tokens", "claude_code.token.input"]),
                    output_tokens: attr_i64(&attrs, &["llm.usage.completion_tokens", "claude_code.token.output"]),
                    cache_creation_tokens: attr_i64(&attrs, &["claude_code.token.cache_creation"]),
                    cache_read_tokens: attr_i64(&attrs, &["claude_code.token.cache_read"]),
                    model: attrs.get("llm.model").or_else(|| attrs.get("claude_code.model")).cloned(),
                    session_id: attrs.get("claude_code.session_id").cloned(),
                    project: attrs.get("claude_code.project")
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
                let points = metric.sum.as_ref().map(|s| &s.data_points)
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
                v.string_value.clone()
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
