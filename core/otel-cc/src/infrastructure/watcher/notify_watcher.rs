use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::application::metrics_cache::MetricsCache;
use crate::application::scan_logs::ScanLogsUseCase;

/// `log_dir` を inotify で監視し、JSONL 変更のたびに ScanLogsUseCase を実行する
pub async fn watch_log_dir(
    log_dir: PathBuf,
    scan_uc: Arc<ScanLogsUseCase>,
    cache: Arc<MetricsCache>,
) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(64);

    // notify は同期 API なので blocking_send でブリッジ
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = tx.blocking_send(res);
    })?;

    watcher.watch(&log_dir, RecursiveMode::Recursive)?;
    info!("Watching: {}", log_dir.display());

    // デバウンス: 短時間に大量イベントが来ても 1 回だけ再スキャン
    let debounce = std::time::Duration::from_secs(2);

    loop {
        let first = rx.recv().await;
        if first.is_none() {
            break; // チャネルが閉じた = watcher が停止
        }

        // debounce 期間中のイベントを消費
        let deadline = tokio::time::Instant::now() + debounce;
        while let Ok(Some(_)) = tokio::time::timeout_at(deadline, rx.recv()).await {}

        // JSONL の変更かどうか確認
        let is_jsonl_change = matches!(
            first.as_ref().and_then(|r| r.as_ref().ok()),
            Some(ev) if matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_))
                && ev.paths.iter().any(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
        );

        if !is_jsonl_change {
            continue;
        }

        info!("JSONL change detected — re-scanning");
        if let Err(e) = scan_uc.run() {
            warn!("Re-scan error: {e}");
        } else if let Ok(summary) = scan_uc.load_summary() {
            cache.update(summary).await;
        }
    }

    // watcher をここまで生存させる（スコープ終了でドロップ）
    drop(watcher);
    Ok(())
}
