# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Purpose

LifeAI は、個人の活動のレコード（日報・振り返り・気づき等）を残していくための AI 対話フレームワークです。Claude Code のスキルシステムを活用し、対話を通じて個人のライフマネジメントを支援します。

## Commands

- `make build` — ワークスペース全体をビルド
- `make test` — テスト実行
- `make fmt` — フォーマット
- `make fmt-check` — フォーマットチェック（CI 用、修正なし）
- `make clippy` — リント（`-D warnings` 付き、警告はすべてエラー扱い）
- `make check` — fmt + clippy + test をまとめて実行
- `make hooks` — Git hooks インストール（`.githooks/` を使用）
- `make init-firewall` — Dev Container 用ファイアウォール初期化（要 root）
- `cargo test -p <crate> <test_name>` — 単一クレートのテスト実行
- `make build-otel-cc` / `make test-otel-cc` — otel-cc クレートのみビルド/テスト
- `make metrics` — otel-cc コンテナの `/metrics` エンドポイントを確認
- `make restart-infra` — otel-cc, prometheus, grafana コンテナを再起動（**ホスト側で実行**）
- `make logs-otel-cc` / `make logs-prometheus` / `make logs-grafana` — 各コンテナのログ表示（**ホスト側で実行**）

## Architecture

### Cargo ワークスペース構成

```
Cargo.toml             ← workspace root (members: core, core/otel-cc)
core/                  ← lifeai-core クレート（ドメイン別サブディレクトリ予約済み）
core/otel-cc/          ← otel-cc クレート（Claude Code 使用状況モニタリング）
.claude/               ← Claude Code スキル・エージェント群
.devcontainer/         ← Dev Container 定義（Rust + Node + Bun）
```

### otel-cc: Claude Code モニタリングシステム

`core/otel-cc/` は Claude Code の使用状況を数値化・可視化する Rust バイナリ。設計詳細は `core/otel-cc/DESIGN.md` を参照。

**データフロー:**
```
~/.claude/projects/*.jsonl ──→ infrastructure/log_reader  ─┐
                                                            ├→ SQLite (/data/otel-cc.db) → /metrics → Prometheus → Grafana
OTLP/HTTP :4318            ──→ infrastructure/otlp_reader  ─┘
```

**4つの並走タスク (main.rs):**
1. `/metrics` HTTP サーバー (:9091) — Prometheus スクレイプ用
2. OTLP/HTTP レシーバー (:4318) — Claude Code OTel 受信
3. inotify ファイル監視 — JSONL 変更を即時検知・差分スキャン
4. 定期スキャン (60秒) — inotify のフォールバック

**Clean Architecture レイヤー構成:**

```
domain/         — エンティティ（Session, TokenEvent 等）、コスト計算、Port トレイト（リポジトリ境界）
infrastructure/ — Port 実装: sqlite/（SqliteRepository）、log_reader/（JSONL パース・差分スキャン）、otlp_reader/（OTLP パース）、watcher/（inotify 監視）
application/    — ユースケース: ScanLogsUseCase, IngestOtlpUseCase, MetricsCache
interface/      — axum HTTP ハンドラー: /metrics（Prometheus）、/v1/{traces,metrics,logs}（OTLP）
```

依存方向: `interface → application → domain ← infrastructure`（domain は外部に依存しない）

**依存性注入 (main.rs がコンポジションルート):**
- `SqliteRepository` を `Arc<dyn SessionPort>` / `Arc<dyn EventPort>` / `Arc<dyn OtlpPort>` として各ユースケースに注入
- SQLite は `std::sync::Mutex<Connection>` で保護（`Sync` を実現するため tokio Mutex は不使用）

**差分スキャンの仕組み:**
- `scan_state` テーブルに `path → lines_processed` を保存
- 再スキャン時は `.skip(lines_processed)` で既処理行を読み飛ばし、重複挿入を防止

**環境変数 (`config.rs`):**

| 変数名 | デフォルト | 説明 |
|---|---|---|
| `OTEL_CC_DB_PATH` | `otel-cc.db` | SQLite DB ファイルパス |
| `OTEL_CC_CLAUDE_LOG_DIR` | `~/.claude/projects` | Claude Code ログディレクトリ |
| `OTEL_CC_OTLP_PORT` | `4318` | OTLP/HTTP 受信ポート |
| `OTEL_CC_METRICS_PORT` | `9091` | Prometheus /metrics 公開ポート |

**Docker Compose インフラ (.devcontainer/compose.yaml):**
- `otel-cc` — volume: `otel-cc-data` (SQLite)
- `prometheus` — volume: `prometheus-data` (90日保持)、`:9090`
- `grafana` — volume: `grafana-data`、`:3000`（認証なし）
- infra 設定: `core/otel-cc/infra/`（prometheus.yml + Grafana provisioning）

**ネットワーク構成（重要）:**
- 全 infra サービスは `network_mode: "service:dev"` で dev コンテナのネットワーク名前空間を共有
- サービス間通信はすべて **`localhost`** 経由（Docker 内部 DNS によるサービス名解決は不可）
- 設定ファイルで他サービスを参照する際は `http://localhost:<port>` を使用すること

**infra 設定ファイル (`core/otel-cc/infra/`):**
```
prometheus.yml                          — スクレイプ設定（localhost:9091/metrics, 15秒間隔）
grafana/provisioning/
  datasources/prometheus.yaml           — Prometheus データソース（localhost:9090）
  dashboards/dashboards.yaml            — ダッシュボードプロバイダー設定
  dashboards/claude-code.json           — Claude Code Monitor ダッシュボード定義
```

**docker/make コマンドはホスト側で実行:** dev コンテナ内には docker CLI がないため、`make restart-infra` や `make logs-*` はホスト側ターミナルから実行する

### スキル・エージェント設計パターン

すべてのスキルは「対話 → 委譲」パターン:

1. **スキル（`.claude/skills/<name>/SKILL.md`）** — ユーザーとの対話型ヒアリング（インライン実行）
2. **サブエージェント（`.claude/agents/<name>-writer.md`）** — ヒアリング結果をもとに成果物を生成（model: opus）

各スキルは `templates/` にテンプレート、`references/` にリファレンス、`assets/` に成果物を格納。

**スキル一覧:**
- `daily-report/` — 対話型日報作成（テキスト + SVG インフォグラフィック）。成果物: `assets/<yyyy-mm-dd>/`
- `idea/` — アイデアブレスト＆構造化（10問ヒアリング → アイデアシート）。成果物: `assets/<theme-name>/`
- `insight-report/` — Claude 使用状況ログ解析（並列サブエージェント4本で分析）
- `gen-skill/` — 対話型スキルスキャフォールド生成。新スキル作成時はこれを使う

### 設計方針

- **コンテキスト境界**: 対話ログや insights 集約時に PII 漏洩・コンテキスト汚染を防ぐため、プロジェクト単位のコンテキスト境界を明確に設ける
- **サブエージェントへの委譲**: スキルは会話履歴を汚染しないよう、構造化されたヒアリング結果のみをサブエージェントに渡す

### Dev Container

`.devcontainer/` で開発環境を定義（Rust, Node.js LTS, Bun）。ファイアウォールによるネットワーク制限あり（`whitelist_domains.conf` で許可ドメインを管理）。ポート転送: `3000, 9090, 9091, 4318`。

## Git Hooks

`.githooks/` に設定済み（`make hooks` でインストール）:
- **pre-commit**: `cargo fmt` + `cargo clippy --all-targets --all-features -- -D warnings`
- **pre-push**: `cargo test`
- **post-commit**: CLAUDE.md 自動更新をバックグラウンド実行（`.claude/scripts/update-claude-md.sh`）。`.rs`/`.toml` ファイル変更時のみ `claude -p` で更新。24h以内の再実行は自動スキップ（`--force` で強制可）

## Coding Conventions

- Rust 命名規約に従う（snake_case for functions/variables, CamelCase for types）
- `clippy -D warnings` を通すこと。将来使用予定のフィールドには `#[allow(dead_code)]` を付ける

## Claude Code Instructions

- 応答は日本語で行うこと
- 既存ファイルの修正には Edit ツールを優先し、Write は新規作成時のみ使用
- ファイル検索には Grep / Glob を使用し、Bash で grep/find/cat を直接実行しない
- コードを読む際は Read ツールを使用し、Bash で cat/head/tail を使わない
- 簡潔に応答し、不要な要約や繰り返しを避ける
- スキルやタスクの作成は対話形式で進める（自動生成禁止）
- スキルやプロンプトの設計時は、ゴール・方針・方向性の定義を最優先で明確化すること
