use anyhow::Result;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tracing::info;

use crate::domain::{
    cost,
    model::{EventSource, ScanState, Session, TokenEvent, ToolCall},
    port::{EventPort, SessionPort},
};
use crate::infrastructure::log_reader::jsonl::{ContentBlock, LogRecord, UserContent};

/// 単一 JSONL ファイルを差分スキャンし、セッション・トークン・ツールコールを記録する
pub fn scan_file(
    path: &Path,
    project: &str,
    session_port: &dyn SessionPort,
    event_port: &dyn EventPort,
) -> Result<()> {
    let path_str = path.to_string_lossy().to_string();
    let mtime = get_mtime(path);

    // 前回スキャン状態の確認
    let state = session_port.get_scan_state(&path_str)?;

    // mtime が変わっていなければスキップ
    if let Some(ref s) = state {
        if s.last_modified == mtime {
            return Ok(());
        }
    }

    // 前回処理済み行数（初回は 0）
    let skip_lines = state.as_ref().map(|s| s.lines_processed).unwrap_or(0);

    info!(
        file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
        skip = skip_lines,
        "scanning"
    );

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    // セッション情報の一時マップ（ファイル内でのみ有効）
    let mut sessions: HashMap<String, Session> = HashMap::new();
    // tool_use_id → (session_id, tool_name, timestamp) のペンディングマップ
    // ※ スキャン境界をまたぐケースは is_error=false として扱う
    let mut pending_tool_calls: HashMap<String, (String, String, String)> = HashMap::new();
    let mut lines_processed = skip_lines;

    for line in reader.lines().skip(skip_lines).map_while(Result::ok) {
        lines_processed += 1;
        let record = match serde_json::from_str::<LogRecord>(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };
        process_record(
            record,
            project,
            &mut sessions,
            &mut pending_tool_calls,
            session_port,
            event_port,
        );
    }

    // スキャン後も pending のままのツールコール = アクティブセッションの未完了ツール
    for (tool_use_id, (sid, tool_name, ts)) in pending_tool_calls {
        let _ = event_port.insert_tool_call(&ToolCall {
            session_id: sid,
            tool_id: Some(tool_use_id),
            timestamp: ts,
            tool_name,
            is_error: false,
            source: EventSource::Log,
        });
    }

    session_port.set_scan_state(
        &path_str,
        &ScanState {
            last_modified: mtime,
            lines_processed,
        },
    )?;

    Ok(())
}

fn process_record(
    record: LogRecord,
    project: &str,
    sessions: &mut HashMap<String, Session>,
    pending_tool_calls: &mut HashMap<String, (String, String, String)>,
    session_port: &dyn SessionPort,
    event_port: &dyn EventPort,
) {
    match record {
        LogRecord::Assistant(a) => {
            let Some(session_id) = a.session_id else {
                return;
            };
            let ts = a.timestamp.clone().unwrap_or_default();

            let entry = sessions
                .entry(session_id.clone())
                .or_insert_with(|| Session {
                    session_id: session_id.clone(),
                    project: project.to_string(),
                    cwd: a.cwd.clone(),
                    git_branch: a.git_branch.clone(),
                    model: a.message.model.clone(),
                    entrypoint: a.entrypoint.clone(),
                    version: a.version.clone(),
                    started_at: ts.clone(),
                    last_seen_at: ts.clone(),
                    is_active: false,
                });
            entry.last_seen_at = ts.clone();
            if let Some(m) = &a.message.model {
                entry.model = Some(m.clone());
            }

            // セッションを DB に記録（最新状態で upsert）
            let _ = session_port.upsert_session(entry);

            // usage → TokenEvent
            if let Some(usage) = &a.message.usage {
                let input = usage.input_tokens.unwrap_or(0);
                let output = usage.output_tokens.unwrap_or(0);
                let cache_create = usage.cache_creation_input_tokens.unwrap_or(0);
                let cache_read = usage.cache_read_input_tokens.unwrap_or(0);
                let model_str = a.message.model.as_deref().unwrap_or("claude-sonnet-4-6");

                let _ = event_port.insert_token_event(&TokenEvent {
                    session_id: session_id.clone(),
                    timestamp: ts.clone(),
                    model: a.message.model.clone(),
                    input_tokens: input,
                    output_tokens: output,
                    cache_creation_tokens: cache_create,
                    cache_read_tokens: cache_read,
                    cost_usd: cost::calculate(model_str, input, output, cache_create, cache_read),
                    source: EventSource::Log,
                });
            }

            // tool_use → ペンディングに積む
            if let Some(content) = &a.message.content {
                for block in content {
                    if let ContentBlock::ToolUse(t) = block {
                        if let (Some(id), Some(name)) = (&t.id, &t.name) {
                            pending_tool_calls
                                .insert(id.clone(), (session_id.clone(), name.clone(), ts.clone()));
                        }
                    }
                }
            }
        }

        LogRecord::User(u) => {
            let Some(session_id) = u.session_id else {
                return;
            };
            let ts = u.timestamp.clone().unwrap_or_default();

            // セッションが未登録の場合は仮登録
            let entry = sessions
                .entry(session_id.clone())
                .or_insert_with(|| Session {
                    session_id: session_id.clone(),
                    project: project.to_string(),
                    cwd: u.cwd.clone(),
                    git_branch: u.git_branch.clone(),
                    model: None,
                    entrypoint: u.entrypoint.clone(),
                    version: u.version.clone(),
                    started_at: ts.clone(),
                    last_seen_at: ts.clone(),
                    is_active: false,
                });
            entry.last_seen_at = ts;
            let _ = session_port.upsert_session(entry);

            // tool_result → ペンディングと照合してツールコールを確定
            if let Some(UserContent::Blocks(blocks)) = &u.message.content {
                for block in blocks {
                    if block.block_type.as_deref() == Some("tool_result") {
                        if let Some(tool_use_id) = &block.tool_use_id {
                            if let Some((sid, tool_name, tool_ts)) =
                                pending_tool_calls.remove(tool_use_id)
                            {
                                let _ = event_port.insert_tool_call(&ToolCall {
                                    session_id: sid,
                                    tool_id: Some(tool_use_id.clone()),
                                    timestamp: tool_ts,
                                    tool_name,
                                    is_error: block.is_error.unwrap_or(false),
                                    source: EventSource::Log,
                                });
                            }
                        }
                    }
                }
            }
        }

        LogRecord::System(s) => {
            // コンテキスト圧縮イベントの検出
            // subtype が "context_compression" または "compacted" を含む場合に記録する
            let is_compression = s
                .subtype
                .as_deref()
                .map(|t| t.contains("compress") || t.contains("compact"))
                .unwrap_or(false);

            if is_compression {
                if let (Some(session_id), Some(ts)) = (s.session_id, s.timestamp) {
                    let _ = session_port.insert_compression_event(
                        &session_id,
                        &ts,
                        s.summary.as_deref(),
                    );
                }
            }
        }

        _ => {}
    }
}

fn get_mtime(path: &Path) -> String {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string()
        })
        .unwrap_or_default()
}
