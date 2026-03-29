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
- `make coverage` — カバレッジ計測（テキストサマリー）
- `make coverage-html` — カバレッジ HTML レポート生成（`target/llvm-cov/html/index.html`）
- `make coverage-check` — カバレッジ計測（60% 未満で失敗）
- `make metrics` — otel-cc コンテナの `/metrics` エンドポイントを確認
- `make restart-infra` — otel-cc, prometheus, grafana コンテナを再起動（**ホスト側で実行**）
- `make rebuild-otel-cc` — otel-cc を再ビルドして再起動（**ホスト側で実行**、Rust コード変更後に必須）
- `make logs-otel-cc` / `make logs-prometheus` / `make logs-grafana` — 各コンテナのログ表示（**ホスト側で実行**）

## Architecture

### Cargo ワークスペース構成

```
Cargo.toml             ← workspace root (members: core, core/otel-cc)
core/                  ← lifeai-core クレート（certifications/, cooking/ 等のドメイン別サブディレクトリ）
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

**5つの並走タスク (main.rs):**
1. `/metrics` HTTP サーバー (:9091) — Prometheus スクレイプ用
2. OTLP/HTTP レシーバー (:4318) — Claude Code OTel 受信
3. inotify ファイル監視 — JSONL 変更を即時検知・差分スキャン
4. 定期スキャン (60秒) — inotify のフォールバック
5. インサイト分析 (5分周期) — 閾値チェック → Grafana アノテーション自動投稿

**Clean Architecture レイヤー構成:**

```
domain/         — エンティティ（Session, TokenEvent, ToolCall 等）、コスト計算、Port トレイト（リポジトリ境界）
infrastructure/ — Port 実装: sqlite/（SqliteRepository）、log_reader/（JSONL パース・差分スキャン）、otlp_reader/（OTLP パース）、watcher/（notify クレートによるファイル監視）
application/    — ユースケース: ScanLogsUseCase, IngestOtlpUseCase, MetricsCache
interface/      — axum HTTP ハンドラー: /metrics（Prometheus）、/health、/api/stats（JSON 統計）、/v1/{traces,metrics,logs}（OTLP）
```

依存方向: `interface → application → domain ← infrastructure`（domain は外部に依存しない）

**依存性注入 (main.rs がコンポジションルート):**
- `SqliteRepository` を `Arc<dyn SessionPort>` / `Arc<dyn EventPort>` / `Arc<dyn OtlpPort>` / `Arc<dyn StatsPort>` として各ユースケースに注入
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
| `OTEL_CC_GRAFANA_URL` | `http://localhost:3000` | Grafana ベース URL（アノテーション送信先） |
| `OTEL_CC_INSIGHT_INTERVAL` | `300` | インサイト分析実行間隔（秒） |
| `OTEL_CC_INSIGHT_COOLDOWN_MIN` | `60` | 同一インサイトの再送クールダウン（分） |

**Docker Compose インフラ (.devcontainer/compose.yaml):**
- `otel-cc` — volume: `otel-cc-data` (SQLite)
- `prometheus` — volume: `prometheus-data` (90日保持)、`:9090`
- `grafana` — volume: `grafana-data`、`:3000`（認証なし）
- infra 設定: `core/otel-cc/infra/`（prometheus.yml + Grafana provisioning）

**ネットワーク構成（重要）:**
- 全 infra サービスは `network_mode: "service:dev"` で dev コンテナのネットワーク名前空間を共有
- サービス間通信はすべて **`localhost`** 経由（Docker 内部 DNS によるサービス名解決は不可）
- 設定ファイルで他サービスを参照する際は `http://localhost:<port>` を使用すること

**HTTP エンドポイント一覧:**

| エンドポイント | 用途 |
|---|---|
| `GET /metrics` | Prometheus テキスト形式 |
| `GET /health` | ヘルスチェック |
| `GET /api/stats` | JSON 統計（`period=N` で直近 N 日、`project=名前` でプロジェクト絞り込み） |
| `POST /v1/traces` `/v1/metrics` `/v1/logs` | OTLP/HTTP 受信 |

`/api/stats` レスポンス構造: `{ overview, projects[], daily[], generated_at }` — insight-report スキルなどがこの API を使って統計を取得する。

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

`.devcontainer/` で開発環境を定義（Rust, Bun, uv/Python, Claude CLI, Codex CLI）。ファイアウォールによるネットワーク制限あり（`whitelist_domains.conf` で許可ドメインを管理）。ポート転送: `3000, 9090, 9091, 4318`。

## Git Hooks

`.githooks/` に設定済み（`make hooks` でインストール）:
- **pre-commit**: `cargo fmt` + `cargo clippy --all-targets --all-features -- -D warnings`
- **pre-push**: `cargo test` + `cargo llvm-cov --fail-under-lines 60`（カバレッジ 60% 未満でプッシュ拒否）
- **post-commit**: バックグラウンドで2つのスクリプトを実行（いずれも24h以内の再実行は自動スキップ、`--force` で強制可）:
  - CLAUDE.md 自動更新（`.claude/scripts/update-claude-md.sh`）— `.rs`/`.toml` ファイル変更時のみ `claude -p` で更新
  - Context Hub ナレッジ抽出（`.claude/skills/insight-report/scripts/context-hub-runner.sh`）— 会話ログからセマンティック知識を抽出

## Coding Conventions

- Rust 命名規約に従う（snake_case for functions/variables, CamelCase for types）
- `clippy -D warnings` を通すこと。将来使用予定のフィールドには `#[allow(dead_code)]` を付ける
- エラーハンドリングは `anyhow::Result` で統一。独自エラー型は不要（`thiserror` は未使用）
- ロギングは `tracing` クレートを使用（`warn!`, `info!`, `debug!` 等）

## otel-cc 新機能追加の手順

Clean Architecture の依存方向に従い、内側から外側へ実装する:

1. **domain/model.rs** — 必要なエンティティ・値オブジェクトを追加
2. **domain/port.rs** — リポジトリ境界となる Port トレイトを定義
3. **infrastructure/sqlite/repository.rs** — Port の SQLite 実装を追加
4. **application/** — ユースケース struct を作成し Port を注入
5. **interface/** — axum ハンドラーからユースケースを呼び出す
6. **main.rs** — 依存性を組み立て（コンポジションルート）

## TDD サイクルと テスト方針

### TDD サイクル（厳守）

このプロジェクトでは **Red → Green → Refactor** サイクルを強制する:

1. **Red**: 失敗するテストを先に書く（実装なしで `cargo test` がコンパイルエラーまたは失敗することを確認）
2. **Green**: テストを通過させる最小限の実装を書く
3. **Refactor**: テストを通したまま設計を改善する

新機能・バグ修正の際は必ずテストを先に書くこと。テストなしの実装 PR は受け付けない。

### カバレッジ目標

- **ライン カバレッジ 60% 以上** を常に維持する
- `make coverage` でサマリー確認、`make coverage-html` で詳細 HTML レポートを確認
- `make coverage-check` は 60% 未満で失敗（pre-push hook でも自動実行）
- 現在のカバレッジ: **~89%**（`main.rs`, `config.rs`, `watcher/` は起動コードのため除外対象）

### テスト記述規則

- ユニットテストは各モジュール末尾の `#[cfg(test)]` ブロックに記述
- `cargo test -p otel-cc <test_name>` で単一テストを実行
- インフラ層（SQLite, JSONL パース）は実ファイル／インメモリ DB を使う統合テストを優先。Port のモック化は原則しない
- `SqliteRepository::with_rollback` を使ってテスト後の状態をクリーンに保つ
- 境界値（NULL, 空リスト, ゼロ除算）を必ずテストする

## スキル新規追加フロー

1. `/gen-skill` スキルを起動してヒアリングを受ける（対話形式）
2. 生成されるファイル構成:
   ```
   .claude/skills/<name>/SKILL.md       ← スキル本体（対話ロジック）
   .claude/agents/<name>-writer.md      ← 成果物生成エージェント（model: opus）
   .claude/skills/<name>/templates/     ← 出力テンプレート
   .claude/skills/<name>/assets/        ← 生成成果物の保存先
   ```
3. スキルは必ずゴール・方針・方向性を冒頭で定義してから実装する

## otel-cc メトリクス解釈ガイド

ユーザーから統計データについて質問されたとき、以下の基準で解釈・改善提案を行うこと。

### 指標の良否判断基準

| 指標 | 良好 | 要注意 | 問題 |
|---|---|---|---|
| セッションあたりコスト | < $8 | $8–$15 | > $15 |
| キャッシュヒット率 | ≥ 95% | 80–95% | < 80% |
| 出力/入力トークン比 | < 5 | 5–10 | > 10 |
| ツールエラー率（任意のツール） | < 5% | 5–10% | > 10% |
| 圧縮イベント / セッション | < 0.2 | 0.2–0.5 | > 0.5 |
| コスト / ツール呼び出し | < $0.05 | $0.05–$0.20 | > $0.20 |

### 各指標が示すもの

- **圧縮イベント / セッション が高い** → セッションが長くなりすぎている。`--resume` で継続するか、タスクを分割する。
- **キャッシュヒット率が低い** → 新規セッションを都度開始している。`--resume` / `--continue` で前セッションを継続すると改善する。
- **出力/入力比が高い** → 応答が冗長。プロンプトに「簡潔に」制約を追加するか、サブエージェントへの出力フォーマット制約を設ける。
- **特定ツールのエラー率が高い（例: Glob, Grep）** → ファイル探索の試行錯誤が多い。2〜3回失敗したら Agent(Explore) に委ねることで削減できる。
- **プロジェクト別コスト/セッションにばらつき** → 高コストプロジェクトでは事前の Plan エージェント設計が不足している可能性がある。

### 「意図との不一致」の近似について

真の意図不一致は測定不能。以下を**間接シグナル**として扱い、**トレンドの急変**を重視すること:
- 圧縮イベント増加（急増 → 異常に長いセッションが発生）
- ツールエラー率の上昇（特定ツールで繰り返し失敗）
- セッションあたりコスト上昇（通常より大幅に高い → 試行錯誤が増えた）

値の絶対値より**前回比・傾向**を見るのが最も有効。

## ワークフロー規約

### セッション継続（コスト最適化）

同じプロジェクトで続けて作業する場合は `claude --resume` または `claude --continue` で前セッションのキャッシュを再利用すること。新セッション開始のたびにキャッシュ作成コストが発生する。特に lifeai プロジェクトは単一セッションあたりのコストが高いため、セッション継続を原則とする。

### worktree での実装前設計

worktree（分離ブランチでの実装タスク）を開始する前に、**必ず Plan エージェントで設計・実装方針を固めてから**実装に入ること。設計なしで直接実装に入ると試行錯誤による無駄なツール呼び出しが増え、コストと時間の両方が膨らむ。

```
Agent(Plan) → 実装方針確定 → worktree で実装開始
```

## Claude Code Instructions

- 応答は日本語で行うこと
- 既存ファイルの修正には Edit ツールを優先し、Write は新規作成時のみ使用
- ファイル検索には Grep / Glob を使用し、Bash で grep/find/cat を直接実行しない
- 同じ系統のファイル探索・コード探索を 2〜3 回繰り返しても目的を達成できない場合、または広範囲にわたる探索が必要な場合は Agent(subagent_type=Explore) に委ねること
- Edit ツールを使う前に、`old_string` がファイル内で一意であることを Grep で確認する。一致が複数ある場合はより広いコンテキストを含めて一意にしてから Edit を実行する
- コードを読む際は Read ツールを使用し、Bash で cat/head/tail を使わない
- 簡潔に応答し、不要な要約や繰り返しを避ける
- タスク管理は TaskCreate / TaskUpdate / TaskList のみ使用する。**TodoWrite は使用禁止**
- スキルやタスクの作成は対話形式で進める（自動生成禁止）
- スキルやプロンプトの設計時は、ゴール・方針・方向性の定義を最優先で明確化すること
