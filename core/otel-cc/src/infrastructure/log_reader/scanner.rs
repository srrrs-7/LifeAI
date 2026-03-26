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

fn get_mtime(path: &Path) -> String {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .to_string()
        })
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::infrastructure::sqlite::SqliteRepository;
    use std::io::Write as _;

    fn repo() -> SqliteRepository {
        SqliteRepository::open(Path::new(":memory:")).unwrap()
    }

    /// assistant レコード 1 行（usage 付き）
    fn assistant_line(session_id: &str, input: i64, output: i64) -> String {
        format!(
            r#"{{"type":"assistant","sessionId":"{session_id}","timestamp":"2026-03-26T10:00:00Z","cwd":"/workspace/proj","gitBranch":"main","entrypoint":"cli","version":"1.0.0","message":{{"model":"claude-sonnet-4-6","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"content":[]}}}}"#
        )
    }

    /// tool_use を含む assistant レコード
    fn tool_use_line(session_id: &str, tool_id: &str, tool_name: &str) -> String {
        format!(
            r#"{{"type":"assistant","sessionId":"{session_id}","timestamp":"2026-03-26T10:00:00Z","cwd":"/w","gitBranch":"main","message":{{"model":"claude-sonnet-4-6","usage":{{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"content":[{{"type":"tool_use","id":"{tool_id}","name":"{tool_name}","input":{{}}}}]}}}}"#
        )
    }

    /// tool_result を含む user レコード
    fn tool_result_line(session_id: &str, tool_id: &str, is_error: bool) -> String {
        format!(
            r#"{{"type":"user","sessionId":"{session_id}","timestamp":"2026-03-26T10:00:01Z","cwd":"/w","gitBranch":"main","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{tool_id}","is_error":{is_error}}}]}}}}"#
        )
    }

    // ── 基本動作 ─────────────────────────────────────────────────

    #[test]
    fn assistant_record_creates_session_and_token_event() {
        let r = repo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, format!("{}\n", assistant_line("sess-1", 100, 50))).unwrap();

        scan_file(&path, "my-project", &r, &r).unwrap();

        let s = r.load_summary().unwrap();
        assert_eq!(s.total_sessions, 1);
        assert_eq!(s.total_input_tokens, 100);
        assert_eq!(s.total_output_tokens, 50);
        assert_eq!(s.projects.first().unwrap().project, "my-project");
    }

    #[test]
    fn tool_use_and_result_pair_recorded_as_tool_call() {
        let r = repo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        let content = format!(
            "{}\n{}\n",
            tool_use_line("sess-1", "toolu_01", "Bash"),
            tool_result_line("sess-1", "toolu_01", false)
        );
        std::fs::write(&path, content).unwrap();

        scan_file(&path, "proj", &r, &r).unwrap();

        let s = r.load_summary().unwrap();
        assert_eq!(s.total_tool_calls, 1);
        assert_eq!(s.total_tool_errors, 0);
        assert!(s.tool_counts.iter().any(|(t, _, _)| t == "Bash"));
    }

    #[test]
    fn error_tool_result_marks_is_error() {
        let r = repo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        let content = format!(
            "{}\n{}\n",
            tool_use_line("sess-1", "toolu_01", "Bash"),
            tool_result_line("sess-1", "toolu_01", true)
        );
        std::fs::write(&path, content).unwrap();

        scan_file(&path, "proj", &r, &r).unwrap();

        let s = r.load_summary().unwrap();
        assert_eq!(s.total_tool_errors, 1);
    }

    // ── 差分スキャン ─────────────────────────────────────────────

    #[test]
    fn scan_state_tracks_lines_processed() {
        let r = repo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, format!("{}\n", assistant_line("s1", 10, 5))).unwrap();

        scan_file(&path, "proj", &r, &r).unwrap();

        let state = r.get_scan_state(&path.to_string_lossy()).unwrap().unwrap();
        assert_eq!(state.lines_processed, 1);
    }

    #[test]
    fn unchanged_mtime_skips_rescan() {
        let r = repo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, format!("{}\n", assistant_line("s1", 100, 50))).unwrap();

        scan_file(&path, "proj", &r, &r).unwrap(); // first scan
        scan_file(&path, "proj", &r, &r).unwrap(); // mtime unchanged → skip

        let s = r.load_summary().unwrap();
        assert_eq!(
            s.total_input_tokens, 100,
            "no double-count on unchanged file"
        );
    }

    #[test]
    fn incremental_scan_processes_only_new_lines() {
        let r = repo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");

        // 初回: 2 行書いてスキャン
        let line1 = assistant_line("s1", 100, 50);
        let line2 = assistant_line("s2", 200, 80);
        std::fs::write(&path, format!("{line1}\n{line2}\n")).unwrap();
        scan_file(&path, "proj", &r, &r).unwrap();

        assert_eq!(r.load_summary().unwrap().total_input_tokens, 300);

        // 10ms 待って mtime を変化させてから 1 行追記
        std::thread::sleep(std::time::Duration::from_millis(10));
        let line3 = assistant_line("s3", 50, 20);
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(f, "{line3}").unwrap();
        drop(f);

        scan_file(&path, "proj", &r, &r).unwrap();

        let s = r.load_summary().unwrap();
        assert_eq!(s.total_sessions, 3);
        assert_eq!(s.total_input_tokens, 350, "only line3's 50 tokens added");
    }

    // ── 堅牢性 ────────────────────────────────────────────────────

    #[test]
    fn malformed_lines_skipped_gracefully() {
        let r = repo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        let content = format!(
            "{}\nnot-valid-json\n{{}}\n{}\n",
            assistant_line("s1", 50, 20),
            assistant_line("s2", 30, 10),
        );
        std::fs::write(&path, content).unwrap();

        scan_file(&path, "proj", &r, &r).unwrap();

        let s = r.load_summary().unwrap();
        assert_eq!(s.total_sessions, 2);
        assert_eq!(s.total_input_tokens, 80);
    }

    #[test]
    fn system_compression_event_recorded() {
        let r = repo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        let compression_line = r#"{"type":"system","sessionId":"sess-1","timestamp":"2026-03-26T10:00:00Z","subtype":"context_compression","summary":"Compressed 5000 tokens"}"#;
        std::fs::write(&path, format!("{compression_line}\n")).unwrap();

        scan_file(&path, "proj", &r, &r).unwrap();

        let s = r.load_summary().unwrap();
        assert_eq!(s.total_compression_events, 1);
    }
}
