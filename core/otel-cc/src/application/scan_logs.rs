use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
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
        Self { session_port, event_port, log_dir }
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
