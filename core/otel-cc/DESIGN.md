# otel-cc 設計書

> Claude Code の使用状況をブラウザで可視化・数値化するモニタリングシステム

---

## 1. 目的と方針

### 目的

- Claude Code のセッションを**リアルタイム**および**履歴ベース**で可視化する
- トークン消費・コスト・ツールコールなどの使用状況を**数値として定量化**する
- Grafana + Prometheus の既存エコシステムを活用し、UI は自作しない

### 方針

- **データソースは2系統**: ローカルログ（バッチ解析）と OTel（リアルタイム）
- **Prometheus 形式で metrics を公開**し、Grafana で可視化する
- **Rust 単一バイナリ**で両系統を担う（外部コレクター不要）
- ログは `~/.claude/projects/` の JSONL を直接パースする（Claude Code 側の設定変更不要）

---

## 2. システム構成

```
┌─────────────────────────────────────────────────────┐
│                   Claude Code                       │
│  セッションログ → ~/.claude/projects/<proj>/*.jsonl │
│  OTel (任意)   → OTLP/HTTP POST :4318              │
└───────────┬──────────────────────┬──────────────────┘
            │ファイル監視(inotify)  │ HTTP
            ▼                      ▼
┌───────────────────────────────────────┐
│             otel-cc (Rust)            │
│                                       │
│  ┌──────────────┐  ┌───────────────┐ │
│  │ log_reader   │  │ otlp_reader   │ │
│  │ (batch+watch)│  │  (realtime)   │ │
│  └──────┬───────┘  └──────┬────────┘ │
│         └────────┬─────────┘         │
│                  ▼                   │
│            MetricsCache              │
│      (RwLock<MetricsSummary>)        │
│                  │                   │
│         GET /metrics :9091           │
└──────────────────┬────────────────────┘
                   │ scrape (15s)
                   ▼
             Prometheus :9090
                   │
                   ▼
             Grafana :3000  ← Browser
```

---

## 3. データソース詳細

### 3-1. ローカルログ（メイン）

**場所**: `~/.claude/projects/<project-slug>/*.jsonl`

**JSONL レコード型**:

```
file-history-snapshot  — ファイルスナップショット（解析対象外）
user                   — ユーザー入力 / tool_result を含む
assistant              — モデル出力 / tool_use を含む / usage あり
system                 — システムプロンプト
progress               — ストリーミング中間状態（解析対象外）
```

**抽出フィールド**:

| フィールド | 場所 | 用途 |
|---|---|---|
| `sessionId` | 全行 | セッション識別 |
| `timestamp` | 全行 | 時刻 |
| `message.model` | assistant | モデル識別 |
| `message.usage.input_tokens` | assistant | 入力トークン |
| `message.usage.output_tokens` | assistant | 出力トークン |
| `message.usage.cache_creation_input_tokens` | assistant | キャッシュ書き込み |
| `message.usage.cache_read_input_tokens` | assistant | キャッシュ読み込み |
| `message.content[].type == "tool_use"` | assistant | ツール呼び出し |
| `message.content[].name` | assistant/tool_use | ツール名 |
| `message.content[].is_error` | user/tool_result | エラー判定 |
| `gitBranch` | 全行 | ブランチ |
| `cwd` | 全行 | プロジェクトパス |
| `entrypoint` | 全行 | cli / ide |
| `version` | 全行 | Claude Code バージョン |

### 3-2. OTel リアルタイム（補助）

Claude Code 側の環境変数を設定した場合のみ有効:

```bash
CLAUDE_CODE_ENABLE_TELEMETRY=1
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
OTEL_EXPORTER_OTLP_PROTOCOL=http/json
```

受信エンドポイント:
- `POST /v1/traces` — スパンデータ
- `POST /v1/metrics` — カウンター / ゲージ
- `POST /v1/logs` — イベントログ

---

## 4. 統計メトリクス定義

### 4-1. セッションレベル

| メトリクス名 | 種別 | ラベル | 意味 |
|---|---|---|---|
| `cc_session_tokens_total` | Counter | `session`, `project`, `model`, `type` | トークン合計 (`type`: input/output/cache_create/cache_read) |
| `cc_session_cost_usd` | Gauge | `session`, `project`, `model` | セッションコスト (USD) |
| `cc_session_duration_seconds` | Gauge | `session`, `project` | セッション経過時間 |
| `cc_session_tool_calls_total` | Counter | `session`, `tool`, `project` | ツール呼び出し回数 |
| `cc_session_tool_errors_total` | Counter | `session`, `tool`, `project` | ツールエラー回数 |
| `cc_session_cache_hit_ratio` | Gauge | `session`, `project` | キャッシュヒット率 |

### 4-2. プロジェクト集計

| メトリクス名 | 種別 | ラベル | 意味 |
|---|---|---|---|
| `cc_project_sessions_total` | Counter | `project`, `model` | セッション数 |
| `cc_project_tokens_total` | Counter | `project`, `model`, `type` | プロジェクト累計トークン |
| `cc_project_cost_usd_total` | Counter | `project`, `model` | プロジェクト累計コスト |
| `cc_project_tool_calls_total` | Counter | `project`, `tool` | ツール別呼び出し総数 |
| `cc_project_error_rate` | Gauge | `project` | ツールエラー率 |

### 4-3. システム全体

| メトリクス名 | 種別 | ラベル | 意味 |
|---|---|---|---|
| `cc_total_cost_usd` | Counter | `model` | 全期間累計コスト |
| `cc_active_sessions` | Gauge | `project` | アクティブセッション数 |
| `cc_model_usage_ratio` | Gauge | `model` | モデル別使用割合 |
| `cc_log_files_analyzed` | Gauge | `project` | 解析済みログファイル数 |

### 4-4. コスト計算式

```
# モデル別単価 (per 1M tokens, 2026年3月時点)
claude-opus-4-6:    input=$15  output=$75  cache_read=$1.5  cache_write=$18.75
claude-sonnet-4-6:  input=$3   output=$15  cache_read=$0.3  cache_write=$3.75
claude-haiku-4-5:   input=$0.25 output=$1.25 cache_read=$0.03 cache_write=$0.3

cost = (input_tokens * input_price
      + output_tokens * output_price
      + cache_read_input_tokens * cache_read_price
      + cache_creation_input_tokens * cache_write_price) / 1_000_000
```

---

## 5. ストレージ設計

### 二層構造

```
OTel受信データ / ログ解析結果
        │
        ▼
  SQLite (永続層)           ← Docker volume でマウント
  /data/otel-cc.db
        │
        │ 起動時ロード / OTLP受信・スキャン都度更新
        ▼
  MetricsCache (hot cache)  ← Prometheus スクレイプ用
  tokio::sync::RwLock<MetricsSummary>
```

- **SQLite** が唯一の真実のソース。プロセス再起動後も全データを保持する
- **MetricsCache** (`application/metrics_cache.rs`) は `/metrics` の高速応答のためのキャッシュ。起動時と更新都度 SQLite から再計算する
- OTel で受信した生スパン・メトリクス・ログも SQLite に記録し、後から再分析できるようにする

### SQLite スキーマ

```sql
-- セッション基本情報
CREATE TABLE IF NOT EXISTS sessions (
    session_id   TEXT PRIMARY KEY,
    project      TEXT NOT NULL,
    cwd          TEXT,
    git_branch   TEXT,
    model        TEXT,
    entrypoint   TEXT,    -- 'cli' | 'ide'
    version      TEXT,    -- Claude Code バージョン
    started_at   TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    is_active    INTEGER DEFAULT 1
);

-- トークン使用量（APIリクエスト単位）
CREATE TABLE IF NOT EXISTS token_events (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id            TEXT NOT NULL,
    timestamp             TEXT NOT NULL,
    model                 TEXT,
    input_tokens          INTEGER DEFAULT 0,
    output_tokens         INTEGER DEFAULT 0,
    cache_creation_tokens INTEGER DEFAULT 0,
    cache_read_tokens     INTEGER DEFAULT 0,
    cost_usd              REAL    DEFAULT 0.0,
    source                TEXT    DEFAULT 'log',  -- 'log' | 'otlp'
    FOREIGN KEY (session_id) REFERENCES sessions(session_id)
);

-- ツールコール
CREATE TABLE IF NOT EXISTS tool_calls (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT NOT NULL,
    tool_id     TEXT,
    timestamp   TEXT NOT NULL,
    tool_name   TEXT NOT NULL,
    is_error    INTEGER DEFAULT 0,
    source      TEXT DEFAULT 'log',
    FOREIGN KEY (session_id) REFERENCES sessions(session_id)
);

-- スキャン状態（JSONL ファイル単位の差分スキャン管理）
CREATE TABLE IF NOT EXISTS scan_state (
    path            TEXT PRIMARY KEY,
    last_modified   TEXT NOT NULL,
    lines_processed INTEGER NOT NULL DEFAULT 0,  -- 処理済み行数（再スキャン時のスキップ用）
    scanned_at      TEXT NOT NULL
);

-- OTel 生スパン（受信データをそのまま保存）
CREATE TABLE IF NOT EXISTS otlp_spans (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    received_at  TEXT NOT NULL,
    trace_id     TEXT,
    span_id      TEXT,
    name         TEXT,
    payload_json TEXT NOT NULL
);

-- OTel 生メトリクス（受信データをそのまま保存）
CREATE TABLE IF NOT EXISTS otlp_metrics (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    received_at  TEXT NOT NULL,
    name         TEXT,
    payload_json TEXT NOT NULL
);

-- OTel 生ログ（受信データをそのまま保存）
CREATE TABLE IF NOT EXISTS otlp_logs (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    received_at  TEXT NOT NULL,
    payload_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_token_events_session ON token_events(session_id);
CREATE INDEX IF NOT EXISTS idx_token_events_time    ON token_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_tool_calls_session   ON tool_calls(session_id);
CREATE INDEX IF NOT EXISTS idx_tool_calls_name      ON tool_calls(tool_name);
```

**差分スキャンの仕組み (`scan_state` テーブル):**

再スキャン時は `lines_processed` 行を `.skip()` で読み飛ばすことで、同一 JSONL ファイルへの重複挿入を防止する。`mtime` のみの比較では行追記を検出できないため、行数ベースの管理に移行した。

### ストレージ構成（コンテナ内パス）

| データ種別 | パス | Volume |
|---|---|---|
| SQLite DB | `/data/otel-cc.db` | `otel-cc-data` |
| Prometheus 時系列 | `/prometheus` | `prometheus-data` |
| Grafana 設定・ダッシュボード | `/var/lib/grafana` | `grafana-data` |

---

## 6. ソースコード構成

Clean Architecture（Domain / Infrastructure / Application / Interface）に従って層を分離。`main.rs` がコンポジションルートとして依存性を組み立てる。

```
core/otel-cc/
├── DESIGN.md               ← 本ファイル
├── Cargo.toml
└── src/
    ├── main.rs             — コンポジションルート（DI 組み立て・4タスク起動）
    ├── config.rs           — 環境変数から読む設定構造体（DB パス、ポート等）
    │
    ├── domain/             — 外部依存ゼロの純粋ドメイン層
    │   ├── model.rs        — エンティティ: Session, TokenEvent, ToolCall,
    │   │                     MetricsSummary, ProjectSummary, ScanState
    │   ├── cost.rs         — モデル別単価定数 + コスト計算関数
    │   └── port.rs         — リポジトリ境界トレイト:
    │                         SessionPort / EventPort / OtlpPort
    │
    ├── infrastructure/     — 外部 I/O を実装するアダプター層
    │   ├── sqlite/
    │   │   └── repository.rs — SqliteRepository（全 Port を実装）
    │   │                       std::sync::Mutex<Connection> で Sync を実現
    │   ├── log_reader/
    │   │   ├── jsonl.rs    — JSONL 型定義 + serde デシリアライズ
    │   │   │                 (AssistantRecord, UserRecord, usage, tool_use 等)
    │   │   └── scanner.rs  — ディレクトリ全走査 + lines_processed 差分スキャン
    │   ├── otlp_reader/
    │   │   └── parser.rs   — OTLP/JSON 型定義 (TracesPayload, MetricsPayload 等)
    │   └── watcher/
    │       └── notify_watcher.rs — notify クレート inotify 監視・2秒デバウンス
    │
    ├── application/        — ユースケース + キャッシュ管理（Port 経由で DB アクセス）
    │   ├── scan_logs.rs    — ScanLogsUseCase: JSONL 走査 → DB 書き込み → サマリー取得
    │   ├── ingest_otlp.rs  — IngestOtlpUseCase: OTLP 受信 → DB 書き込み
    │   └── metrics_cache.rs — MetricsCache: RwLock<MetricsSummary> hot cache
    │
    └── interface/          — HTTP ハンドラー + Prometheus レンダリング
        ├── otlp_handler.rs — axum ルーター: POST /v1/{traces,metrics,logs}
        ├── metrics_handler.rs — GET /metrics (キャッシュから取得)
        └── prometheus.rs   — MetricsSummary → Prometheus テキスト形式変換
```

**依存方向**: `interface` → `application` → `domain` ← `infrastructure`
（infrastructure は domain の Port を実装するが、application/interface には依存しない）

---

## 7. インフラ構成（Docker Compose 追加）

```yaml
# .devcontainer/compose.yaml に追加するサービス

  otel-cc:
    build:
      context: ..
      dockerfile: core/otel-cc/Dockerfile
    volumes:
      - ~/.claude:/home/vscode/.claude:ro  # Claudeログ読み取り専用
      - otel-cc-data:/data                 # SQLite 永続化
    environment:
      - OTEL_CC_DB_PATH=/data/otel-cc.db
      - OTEL_CC_CLAUDE_LOG_DIR=/home/vscode/.claude/projects
      - OTEL_CC_OTLP_PORT=4318
      - OTEL_CC_METRICS_PORT=9091
    ports:
      - "4318:4318"   # OTLP/HTTP 受信
      - "9091:9091"   # /metrics (Prometheus スクレイプ)
    restart: unless-stopped

  prometheus:
    image: prom/prometheus:v3
    volumes:
      - ../core/otel-cc/infra/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus        # 時系列データ永続化
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=90d'  # 90日間保持
    ports:
      - "9090:9090"
    restart: unless-stopped

  grafana:
    image: grafana/grafana:11
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Admin
      - GF_DASHBOARDS_DEFAULT_HOME_DASHBOARD_PATH=/etc/grafana/provisioning/dashboards/claude-code.json
    volumes:
      - ../core/otel-cc/infra/grafana/provisioning:/etc/grafana/provisioning:ro
      - grafana-data:/var/lib/grafana      # ダッシュボード・設定永続化
    ports:
      - "3000:3000"
    restart: unless-stopped

# --- named volumes ---
volumes:
  otel-cc-data:      # otel-cc SQLite DB
  prometheus-data:   # Prometheus 時系列データ
  grafana-data:      # Grafana 設定・ダッシュボード
```

`devcontainer.json` の `forwardPorts` に `3000`, `9090`, `9091`, `4318` を追加する。

### インフラ設定ファイル構成

```
core/otel-cc/infra/
├── prometheus.yml                        — Prometheus スクレイプ設定
└── grafana/
    └── provisioning/
        ├── datasources/
        │   └── prometheus.yaml           — データソース自動設定
        └── dashboards/
            ├── dashboards.yaml           — ダッシュボード読み込み設定
            └── claude-code.json          — Claude Code ダッシュボード定義
```

---

## 8. Grafana ダッシュボード構成

### Row 1: 現在の状態
- アクティブセッション数
- 本日のトークン合計（input / output / cache）
- 本日の累計コスト (USD)
- キャッシュヒット率

### Row 2: トークン分析
- 時系列: トークン消費量（セッション別、積み上げ）
- 棒グラフ: モデル別トークン使用割合
- ゲージ: cache_read / total_input の比率

### Row 3: ツール分析
- 棒グラフ: ツール別呼び出し回数 (Bash / Read / Edit / Glob / Grep / Agent...)
- 棒グラフ: ツール別エラー率
- 時系列: エラー率推移

### Row 4: コスト管理
- 時系列: 日別累計コスト
- テーブル: セッション一覧（コスト降順）
- スタット: 週次/月次コスト合計

---

## 9. 実装フェーズ

### Phase 1: ログ解析基盤 + 永続化 ✅ 完了
- SQLite スキーマ初期化（WAL モード、7テーブル）
- JSONL パーサーとセッション統計算出（差分スキャン: `lines_processed` ベース）
- SQLite への書き込みと MetricsCache への反映
- `/metrics` エンドポイント（Prometheus テキスト形式）
- Docker Compose: Prometheus + Grafana + named volumes 追加
- infra 設定ファイル一式（prometheus.yml, grafana provisioning）
- Clean Architecture リファクタリング（domain / infrastructure / application / interface）

### Phase 2: リアルタイム対応 ✅ 完了
- OTLP/HTTP レシーバー（`/v1/traces`, `/v1/metrics`, `/v1/logs`）
- 受信データを `otlp_spans` / `otlp_metrics` / `otlp_logs` テーブルに保存
- inotify によるログファイル変更監視（2秒デバウンス）

### Phase 3: 高度化（未着手）
- 基本 Grafana ダッシュボード JSON の作成・provisioning
- コンテキスト圧縮イベントの検出と可視化
- アラート設定（Grafana Alerting: コスト上限・エラー率閾値）
- SQLite の過去データを Prometheus へ backfill する機能

---

## 10. 未解決事項

| 項目 | 状況 |
|---|---|
| コンテキスト圧縮イベントの検出精度 | `system` レコードの `subtype` フィールドに "compress"/"compact" を含む場合に記録。実際のログで `subtype` 名を検証・調整が必要 |
| ツールコール境界をまたいだスキャン | スキャン境界をまたぐ `tool_use` は `is_error: false` で記録（設計上の許容済み制約）|

### 解決済み

| 項目 | 解決内容 |
|---|---|
| Grafana ダッシュボード JSON | 4 Row 構成・datasource 変数・timeseries パネル付きで実装済み |
| OTel トークンカテゴリ分類 | JSONL ログ解析で代替。`source='otlp'` と `source='log'` を `token_events` テーブルで区別 |
