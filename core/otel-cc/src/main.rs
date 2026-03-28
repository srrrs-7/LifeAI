mod application;
mod config;
mod domain;
mod infrastructure;
mod interface;

use anyhow::Result;
use axum::{routing::get, Router};
use std::sync::Arc;
use tracing::info;

use application::{
    ingest_otlp::IngestOtlpUseCase,
    insight_analysis::InsightAnalysisUseCase,
    metrics_cache::MetricsCache,
    scan_logs::ScanLogsUseCase,
};
use config::Config;
use infrastructure::{
    grafana::GrafanaAnnotationClient, sqlite::SqliteRepository, watcher::watch_log_dir,
};
use interface::{metrics_handler, otlp_handler::OtlpState, stats_handler};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "otel_cc=info".parse().unwrap()),
        )
        .init();

    let config = Config::from_env();
    info!("DB: {}", config.db_path.display());
    info!("Claude logs: {}", config.claude_log_dir.display());

    // ── 依存性の組み立て ─────────────────────────────────────────
    let repo = Arc::new(SqliteRepository::open(&config.db_path)?);
    let cache = Arc::new(MetricsCache::new());

    let scan_uc = Arc::new(ScanLogsUseCase::new(
        repo.clone() as Arc<dyn domain::port::SessionPort>,
        repo.clone() as Arc<dyn domain::port::EventPort>,
        config.claude_log_dir.clone(),
    ));

    let otlp_uc = Arc::new(IngestOtlpUseCase::new(
        repo.clone() as Arc<dyn domain::port::SessionPort>,
        repo.clone() as Arc<dyn domain::port::EventPort>,
        repo.clone() as Arc<dyn domain::port::OtlpPort>,
    ));

    let grafana_client = Arc::new(GrafanaAnnotationClient::new(&config.grafana_url));
    let insight_uc = Arc::new(InsightAnalysisUseCase::new(
        repo.clone() as Arc<dyn domain::port::SessionPort>,
        grafana_client,
        repo.clone() as Arc<dyn domain::port::InsightStatePort>,
        config.insight_cooldown_minutes,
    ));

    // ── 起動時スキャン ───────────────────────────────────────────
    scan_uc.run()?;
    if let Ok(summary) = scan_uc.load_summary() {
        cache.update(summary).await;
    }
    info!("Initial scan complete");

    // ── Task 1: Prometheus /metrics + /api/stats サーバー ────────
    let metrics_addr = format!("0.0.0.0:{}", config.metrics_port);
    let metrics_cache = cache.clone();
    let stats_port = repo.clone() as Arc<dyn domain::port::StatsPort>;
    tokio::spawn(async move {
        let app = Router::new()
            .route("/metrics", get(metrics_handler::handle))
            .route("/health", get(|| async { "ok" }))
            .route(
                "/api/stats",
                get(stats_handler::handle).with_state(stats_port),
            )
            .with_state(metrics_cache);
        info!("Metrics server: http://{metrics_addr}/metrics");
        info!("Stats API:      http://{metrics_addr}/api/stats");
        let listener = tokio::net::TcpListener::bind(&metrics_addr)
            .await
            .expect("bind metrics port");
        axum::serve(listener, app)
            .await
            .expect("metrics server error");
    });

    // ── Task 2: OTLP/HTTP レシーバー ─────────────────────────────
    let otlp_addr = format!("0.0.0.0:{}", config.otlp_port);
    let otlp_state = OtlpState {
        use_case: otlp_uc,
        session_port: repo.clone() as Arc<dyn domain::port::SessionPort>,
        cache: cache.clone(),
    };
    tokio::spawn(async move {
        let app = interface::otlp_handler::router(otlp_state);
        info!("OTLP receiver: http://{otlp_addr}/v1/{{traces,metrics,logs}}");
        let listener = tokio::net::TcpListener::bind(&otlp_addr)
            .await
            .expect("bind OTLP port");
        axum::serve(listener, app).await.expect("OTLP server error");
    });

    // ── Task 3: inotify ファイル監視 ─────────────────────────────
    let watch_dir = config.claude_log_dir.clone();
    let watch_scan = scan_uc.clone();
    let watch_cache = cache.clone();
    tokio::spawn(async move {
        if let Err(e) = watch_log_dir(watch_dir, watch_scan, watch_cache).await {
            tracing::warn!("File watcher stopped: {e}");
        }
    });

    // ── Task 4: 定期スキャン（inotify のフォールバック） ─────────
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = scan_uc.run() {
                tracing::warn!("Periodic scan error: {e}");
            } else if let Ok(summary) = scan_uc.load_summary() {
                cache.update(summary).await;
            }
        }
    });

    // ── Task 5: インサイト分析 & Grafana アノテーション ──────────
    let insight_interval = config.insight_interval_secs;
    tokio::spawn(async move {
        // 起動直後は少し待ってから最初の分析を実行
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(insight_interval));
        loop {
            interval.tick().await;
            if let Err(e) = insight_uc.run().await {
                tracing::warn!("Insight analysis error: {e}");
            }
        }
    });

    tokio::signal::ctrl_c().await?;
    info!("Shutting down");
    Ok(())
}
