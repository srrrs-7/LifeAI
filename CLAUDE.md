# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Purpose

**解決する課題:** Claude Code を日常的に使う開発者・チームの「活動記録が形に残らない」「使い方が効率的かどうかわからない」を解決する。

- **スキルシステム**: 対話形式のヒアリングで日々の知的作業を構造化された成果物（日報・アイデアシート・ブログ記事）に変換する
- **otel-cc モニタリング**: Claude Code のセッションログを自動解析し、コスト・効率・異常を Grafana ダッシュボードで可視化する（個人・チーム両対応）

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
- `make build-release` — リリースビルド
- `make clean` — `target/` を削除
- `make open-grafana` / `make open-prometheus` — Grafana / Prometheus の URL 表示

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
domain/         — エンティティ（Session, TokenEvent, ToolCall 等）、コスト計算（cost.rs）、トレンド分析（trend.rs）、Port トレイト（リポジトリ境界）
infrastructure/ — Port 実装: sqlite/（SqliteRepository）、log_reader/（JSONL パース・差分スキャン）、otlp_reader/（OTLP パース）、watcher/（notify クレートによるファイル監視）、grafana/（GrafanaAnnotationClient）
application/    — ユースケース: ScanLogsUseCase, IngestOtlpUseCase, InsightAnalysisUseCase, MetricsCache, AnalyticsUseCase, BenchmarkUseCase, CostOptimizationUseCase
interface/      — axum HTTP ハンドラー: /metrics、/health、/api/stats、/api/analytics、/api/optimization、/api/benchmarks、/v1/{traces,metrics,logs}（OTLP）
```

依存方向: `interface → application → domain ← infrastructure`（domain は外部に依存しない）

**依存性注入 (main.rs がコンポジションルート):**
- `SqliteRepository` を `Arc<dyn SessionPort>` / `Arc<dyn EventPort>` / `Arc<dyn OtlpPort>` / `Arc<dyn StatsPort>` / `Arc<dyn InsightStatePort>` / `Arc<dyn TrendDataPort>` / `Arc<dyn AnalyticsPort>` / `Arc<dyn BenchmarkPort>` として各ユースケースに注入
- `GrafanaAnnotationClient` を `Arc<dyn AnnotationPort>` として `InsightAnalysisUseCase` に注入
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
| `OTEL_CC_INSIGHT_DAILY_COST_ALERT` | `10.0` | 日次コストアラート閾値（USD）。超過で Grafana アノテーション送信 |
| `OTEL_CC_INSIGHT_COST_WARN` | `3.0` | セッションあたりコスト Warning 閾値（USD） |
| `OTEL_CC_INSIGHT_COST_ALERT` | `8.0` | セッションあたりコスト Alert 閾値（USD） |
| `OTEL_CC_INSIGHT_TOOL_ERROR_WARN` | `0.05` | ツールエラー率 Warning 閾値 |
| `OTEL_CC_INSIGHT_TOOL_ERROR_ALERT` | `0.10` | ツールエラー率 Alert 閾値 |
| `OTEL_CC_INSIGHT_TOOL_MIN_CALLS` | `5` | ツールエラー率を計算する最小呼び出し数 |
| `OTEL_CC_INSIGHT_CACHE_WARN` | `0.90` | キャッシュヒット率 Warning 閾値（この値を下回ると警告） |
| `OTEL_CC_INSIGHT_CACHE_ALERT` | `0.50` | キャッシュヒット率 Alert 閾値 |
| `OTEL_CC_INSIGHT_TREND_LOOKBACK` | `14` | トレンド分析の過去参照日数 |
| `OTEL_CC_INSIGHT_TREND_HORIZON` | `7.0` | トレンド予測の対象日数 |
| `OTEL_CC_USER` | OS ユーザー名 | ユーザー識別名（チーム内で一意にする） |

**Docker Compose インフラ:**

| 構成 | ファイル | ネットワーク | サービス間通信 |
|---|---|---|---|
| 開発（Dev Container） | `.devcontainer/compose.yaml` | `network_mode: "service:dev"` | **`localhost`** 経由 |
| チーム共有 | `core/otel-cc/infra/docker-compose.team.yaml` | 標準 bridge | **サービス名** DNS（`otel-cc:9091` 等） |

開発環境: `otel-cc` + `prometheus` + `grafana`（volume で SQLite / Prometheus / Grafana データを永続化）

**ネットワーク構成（重要）:**
- **開発環境**: `network_mode: "service:dev"` により全サービスが dev コンテナのネットワーク名前空間を共有。設定ファイルでは `http://localhost:<port>` を使用
- **チーム環境**: 標準 bridge ネットワークのため、設定ファイルではサービス名（`http://prometheus:9090` 等）を使用。開発環境とは設定が異なるため、`prometheus-team.yml` と `grafana-team/` に分離している

**HTTP エンドポイント一覧:**

| エンドポイント | 用途 |
|---|---|
| `GET /metrics` | Prometheus テキスト形式 |
| `GET /health` | ヘルスチェック |
| `GET /api/stats` | JSON 統計（`period=N` で直近 N 日、`project=名前` / `user=名前` でフィルタ） |
| `GET /api/analytics` | 分析データ（ツール使用分析、セッション効率等） |
| `GET /api/optimization` | コスト最適化提案 |
| `GET /api/benchmarks` | ベンチマーク比較データ |
| `POST /v1/traces` `/v1/metrics` `/v1/logs` | OTLP/HTTP 受信 |

`/api/stats` レスポンス構造: `{ overview, projects[], users[], daily[], generated_at }` — insight-report スキルなどがこの API を使って統計を取得する。

**Prometheus メトリクス体系:**
- **既存メトリクス** (`cc_sessions`, `cc_cost_usd`, `cc_tokens_total` 等): チーム全体の集約値。後方互換
- **ユーザー別メトリクス** (`cc_user_sessions{user="alice"}`, `cc_user_cost_usd{user="alice"}` 等): `user` ラベル付きで個人別内訳を提供

**infra 設定ファイル (`core/otel-cc/infra/`):**
```
prometheus.yml                          — 開発用スクレイプ設定（localhost:9091/metrics, 15秒間隔）
prometheus-team.yml                     — チーム用スクレイプ設定（otel-cc:9091）
docker-compose.team.yaml                — チーム共有デプロイ構成
grafana/provisioning/                   — 開発用・チーム共通ダッシュボード（localhost ベース）
  dashboards/claude-code.json           — Claude Code Monitor（総合 + Team Overview 行）
  dashboards/session-efficiency.json    — セッション効率分析
  dashboards/cost-management.json       — コスト管理（Today's Snapshot, 日別トレンド, モデル別, 予算追跡）
  dashboards/tool-analytics.json        — ツール分析（ランキング, エラー, アンチパターン）
  dashboards/periodic-review.json       — 定期振り返り（期間比較, ローリング平均, 累積）
  dashboards/model-optimization.json    — モデル最適化（分布, 効率比較, トークン効率）
  dashboards/anomaly-detection.json     — 異常検知（±2σ バンド, スパイク検出）
grafana-team/provisioning/              — チーム用 Grafana（サービス名ベース、datasource設定のみ異なる）
```

全ダッシュボードに `$user` テンプレート変数あり（`label_values(cc_user_sessions, user)`、includeAll: true）。

**docker/make コマンドはホスト側で実行:** dev コンテナ内には docker CLI がないため、`make restart-infra` や `make logs-*` はホスト側ターミナルから実行する

### スキル・エージェント設計パターン

すべてのスキルは「対話 → 委譲」パターン:

1. **スキル（`.claude/skills/<name>/SKILL.md`）** — ユーザーとの対話型ヒアリング（インライン実行）
2. **サブエージェント（`.claude/agents/<name>-writer.md`）** — ヒアリング結果をもとに成果物を生成（model: opus）

各スキルは `templates/` にテンプレート、`references/` にリファレンス、`assets/` に成果物を格納。

**スキル一覧:**
- `daily-report/` — 対話型日報作成（テキスト + SVG インフォグラフィック）。成果物: `assets/<yyyy-mm-dd>/`
- `idea/` — アイデアブレスト＆構造化（10問ヒアリング → アイデアシート）。成果物: `assets/<theme-name>/`
- `blog/` — 対話型ブログ記事作成（ヒアリング → Markdown 記事）
- `commit/` — Git コミット自動化（差分から Conventional Commits 形式のメッセージを自動生成）
- `grafana-report/` — otel-cc `/api/stats` API から統計取得し KPI 分析・改善提案を Markdown 出力
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

## otel-cc コスト計算（domain/cost.rs）

モデル名文字列からバージョンを判別し、USD 単価（per 1M tokens）を適用する:

| モデル | input | output | cache_write | cache_read | マッチ条件 |
|---|---|---|---|---|---|
| Opus 4.5/4.6 | $5 | $25 | $6.25 | $0.50 | `contains("opus-4-5")` or `contains("opus-4-6")` |
| Opus 4.0/4.1 (legacy) | $15 | $75 | $18.75 | $1.50 | `contains("opus")` かつ上記以外 |
| Sonnet (全バージョン) | $3 | $15 | $3.75 | $0.30 | デフォルト（opus/haiku 以外） |
| Haiku 4.5 | $1 | $5 | $1.25 | $0.10 | `contains("haiku")` かつ 3.5/3 以外 |
| Haiku 3.5 | $0.80 | $4 | $1.00 | $0.08 | `contains("haiku-3-5")` |
| Haiku 3 (deprecated) | $0.25 | $1.25 | $0.30 | $0.03 | `contains("haiku-3")` or `contains("haiku-20")` |

cache_write = 1.25x input、cache_read = 0.1x input。新モデル追加時はレート定数とマッチ条件の両方を更新すること。

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
- 現在のカバレッジ: **~90%**（158テスト。`main.rs`, `config.rs`, `watcher/` は起動コードのため除外対象）

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

> **予算前提:** 月上限 $200（日次目標 $6.67、日次アラート $10）

| 指標 | 良好 | 要注意 | 問題 |
|---|---|---|---|
| 日次コスト | < $7 | $7–$10 | > $10 |
| セッションあたりコスト | < $3 | $3–$8 | > $8 |
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

### 実装前設計（コスト削減の最重要施策）

**3ファイル以上の変更が見込まれるタスク**は、実装前に必ず Plan エージェントで設計を固める。worktree に限らず、メインブランチでの作業にも適用する。設計なしで直接実装に入ると試行錯誤による無駄なツール呼び出しが増え、コストと時間の両方が膨らむ。

### モデル使い分け（必須）

**Plan は Opus、実装は Sonnet** でモデルを分離すること。Opus の高い推論能力を設計に集中させ、実装の大量トークン消費を Sonnet のコスト効率で抑える。

```
Agent(Plan, model=opus) → 実装方針確定 → Agent(実装, model=sonnet)
```

| フェーズ | モデル | 理由 |
|---|---|---|
| 設計・Plan | **Opus** | 複雑な依存関係の分析、アーキテクチャ判断、エッジケース洗い出しに高い推論力が必要 |
| 実装・コーディング | **Sonnet** | 計画が明確なら Sonnet で十分。出力トークン単価が Opus の 60% で済む |
| コードレビュー・リファクタ | **Sonnet** | 既存コードの改善は定型的な判断が多い |
| 探索・調査 | **Sonnet** または **Haiku** | 単純な検索・ファイル読み取りに Opus は過剰 |

**実装例:**
```
// 設計（Opus）
Agent(subagent_type=Plan, model=opus, prompt="...")

// 実装（Sonnet）
Agent(model=sonnet, prompt="計画に基づいて実装: ...")
```

設計フェーズで確認すべき項目:
- 変更対象ファイルの一覧と依存関係
- テストの追加・変更方針
- 既存のコードパターンとの整合性

## Claude Code Instructions

- 応答は日本語で行うこと
- 既存ファイルの修正には Edit ツールを優先し、Write は新規作成時のみ使用
- ファイル検索には Grep / Glob を使用し、Bash で grep/find/cat を直接実行しない
- 同じ系統のファイル探索・コード探索を 2〜3 回繰り返しても目的を達成できない場合、または広範囲にわたる探索が必要な場合は Agent(subagent_type=Explore) に委ねること
- Edit ツールを使う前に、`old_string` がファイル内で一意であることを Grep で確認する。一致が複数ある場合はより広いコンテキストを含めて一意にしてから Edit を実行する
- コードを読む際は Read ツールを使用し、Bash で cat/head/tail を使わない
- **Read の効率化:** 大きなファイル（200行超）を読む際は `offset` と `limit` を指定して必要な部分だけ読む。ファイル全体を読んでから目的の箇所を探す方法は避ける。事前に Grep で行番号を特定してからピンポイントで Read する
- **ツール呼び出しの最小化:** 同じファイルの複数箇所を確認する場合、1回の Read で十分な範囲をカバーする。Grep → Read の2段階で済む場面で Grep → Read → Read → Read と繰り返さない
- **簡潔に応答する（出力トークン最適化）:**
  - 完了報告や要約の繰り返しは不要。diff やツール結果がユーザーに見えるため、同じ内容を文章で繰り返さない
  - コード変更後に「変更内容のサマリー」を長文で出力しない。変更点は箇条書き1〜3行で十分
  - サブエージェントへの委譲時、プロンプトに「結果のみ返す。説明・前置き・要約は不要」制約を含める
  - 選択肢の提示は3つ以内。網羅的な列挙より最適な1案の提示を優先する
- タスク管理は TaskCreate / TaskUpdate / TaskList のみ使用する。**TodoWrite は使用禁止**
- スキルやタスクの作成は対話形式で進める（自動生成禁止）
- スキルやプロンプトの設計時は、ゴール・方針・方向性の定義を最優先で明確化すること
