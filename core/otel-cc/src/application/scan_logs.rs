use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::warn;

use crate::domain::{
    model::MetricsSummary,
    port::{EventPort, SessionPort},
};
use crate::infrastructure::log_reader::scanner::scan_file;

pub struct ScanLogsUseCase {
    session_port: Arc<dyn SessionPort>,
    event_port: Arc<dyn EventPort>,
    log_dir: PathBuf,
}

impl ScanLogsUseCase {
    pub fn new(
        session_port: Arc<dyn SessionPort>,
        event_port: Arc<dyn EventPort>,
        log_dir: PathBuf,
    ) -> Self {
        Self {
            session_port,
            event_port,
            log_dir,
        }
    }

    /// 全プロジェクトの JSONL を差分スキャンする
    pub fn run(&self) -> Result<()> {
        if !self.log_dir.exists() {
            warn!("Claude log dir not found: {}", self.log_dir.display());
            return Ok(());
        }

        for project_entry in std::fs::read_dir(&self.log_dir)?.flatten() {
            let project_path = project_entry.path();
            if !project_path.is_dir() {
                continue;
            }

            let project_name = project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            for file_entry in std::fs::read_dir(&project_path)?.flatten() {
                let file_path = file_entry.path();
                if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }

                if let Err(e) = scan_file(
                    &file_path,
                    &project_name,
                    self.session_port.as_ref(),
                    self.event_port.as_ref(),
                ) {
                    warn!("Failed to scan {}: {e}", file_path.display());
                }
            }
        }

        Ok(())
    }

    /// スキャン後のサマリーを取得する
    pub fn load_summary(&self) -> Result<MetricsSummary> {
        self.session_port.load_summary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::sqlite::SqliteRepository;
    use std::path::Path;
    use std::sync::Arc;

    fn repo() -> Arc<SqliteRepository> {
        Arc::new(SqliteRepository::open(Path::new(":memory:")).unwrap())
    }

    fn use_case(dir: &std::path::Path) -> (Arc<SqliteRepository>, ScanLogsUseCase) {
        let r = repo();
        let uc = ScanLogsUseCase::new(r.clone(), r.clone(), dir.to_path_buf());
        (r, uc)
    }

    fn assistant_line(session_id: &str, input: i64, output: i64) -> String {
        format!(
            r#"{{"type":"assistant","sessionId":"{session_id}","timestamp":"2026-01-01T00:00:00Z","cwd":"/w","gitBranch":"main","message":{{"model":"claude-sonnet-4-6","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"content":[]}}}}"#
        )
    }

    // ── 基本動作 ──────────────────────────────────────────────

    #[test]
    fn nonexistent_log_dir_returns_ok() {
        let (_, uc) = use_case(Path::new("/no/such/dir/at/all"));
        assert!(uc.run().is_ok());
    }

    #[test]
    fn scans_jsonl_in_project_subdirs() {
        let dir = tempfile::tempdir().unwrap();
        let proj_dir = dir.path().join("my-project");
        std::fs::create_dir(&proj_dir).unwrap();
        std::fs::write(
            proj_dir.join("session.jsonl"),
            format!("{}\n", assistant_line("s1", 100, 50)),
        )
        .unwrap();

        let (r, uc) = use_case(dir.path());
        uc.run().unwrap();

        let s = r.load_summary().unwrap();
        assert_eq!(s.total_sessions, 1);
        assert_eq!(s.total_input_tokens, 100);
        // プロジェクト名はディレクトリ名から
        assert_eq!(s.projects.first().unwrap().project, "my-project");
    }

    #[test]
    fn non_jsonl_files_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let proj_dir = dir.path().join("proj");
        std::fs::create_dir(&proj_dir).unwrap();
        std::fs::write(proj_dir.join("notes.txt"), "not scanned").unwrap();
        std::fs::write(
            proj_dir.join("ok.jsonl"),
            format!("{}\n", assistant_line("s1", 10, 5)),
        )
        .unwrap();

        let (r, uc) = use_case(dir.path());
        uc.run().unwrap();

        // notes.txt は無視され、ok.jsonl だけ処理される
        assert_eq!(r.load_summary().unwrap().total_sessions, 1);
    }

    #[test]
    fn top_level_files_not_scanned_only_subdirs() {
        let dir = tempfile::tempdir().unwrap();
        // ログディレクトリ直下のファイルは project サブディレクトリ扱いにならない
        std::fs::write(
            dir.path().join("direct.jsonl"),
            format!("{}\n", assistant_line("s1", 99, 1)),
        )
        .unwrap();

        let (r, uc) = use_case(dir.path());
        uc.run().unwrap();

        assert_eq!(r.load_summary().unwrap().total_sessions, 0);
    }

    #[test]
    fn multiple_projects_scanned_independently() {
        let dir = tempfile::tempdir().unwrap();
        // セッション ID を異なるものにして upsert 重複排除を避ける
        for (proj, sid) in &[("alpha", "sess-alpha"), ("beta", "sess-beta")] {
            let pd = dir.path().join(proj);
            std::fs::create_dir(&pd).unwrap();
            std::fs::write(
                pd.join("s.jsonl"),
                format!("{}\n", assistant_line(sid, 50, 10)),
            )
            .unwrap();
        }

        let (r, uc) = use_case(dir.path());
        uc.run().unwrap();

        let s = r.load_summary().unwrap();
        assert_eq!(s.total_sessions, 2);
        assert_eq!(s.total_input_tokens, 100);
    }
}
