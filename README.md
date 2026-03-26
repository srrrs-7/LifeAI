# LifeAI

個人の活動記録（日報・振り返り・気づき等）を残していくための AI 対話フレームワーク。Claude Code のスキルシステムを活用し、対話を通じて個人のライフマネジメントを支援する。

## セットアップ

**前提条件:** Docker, Docker Compose, VS Code + Dev Containers 拡張

```bash
# Dev Container 起動後
make hooks    # Git hooks インストール（cargo fmt/clippy/test を自動実行）
make build    # ビルド確認
```

## スキル一覧

| スキル | 呼び出し方 | 成果物 |
|---|---|---|
| `daily-report` | `/daily-report` | 日報 Markdown + SVG インフォグラフィック（`assets/<yyyy-mm-dd>/`） |
| `idea` | `/idea` | 構造化アイデアシート（`assets/<theme-name>/`） |
| `insight-report` | `/insight-report` | Claude 使用状況分析レポート（並列4エージェント） |
| `gen-skill` | `/gen-skill` | 新規スキルの雛形一式 |

## Claude Code モニタリング (`otel-cc`)

Claude Code の使用状況（トークン・コスト・ツールコール）を数値化し、Grafana でブラウザから確認できるモニタリングシステム。

```bash
docker compose -f .devcontainer/compose.yaml up otel-cc prometheus grafana
```

| URL | 用途 |
|---|---|
| `http://localhost:3000` | Grafana ダッシュボード |
| `http://localhost:9091/metrics` | Prometheus メトリクス直接確認 |

**OTel リアルタイム受信を有効にする場合 (`~/.claude/settings.json`):**

```json
{
  "env": {
    "CLAUDE_CODE_ENABLE_TELEMETRY": "1",
    "OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:4318",
    "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json"
  }
}
```

設定なしでも `~/.claude/projects/` のローカルログを自動解析する。

### ログ収集の仕組み

Claude Code のセッションログ（JSONL）は **3段構え**で収集される:

| トリガー | タイミング | 役割 |
|---|---|---|
| 起動時フルスキャン | バイナリ起動直後 | 初回データ取り込み |
| inotify ファイル監視 | JSONL 変更検知 → 2秒デバウンス後 | リアルタイム収集 |
| 定期スキャン | 60秒間隔 | inotify 取りこぼしのフォールバック |

各スキャンでは **mtime + 行オフセット** による差分処理を行い、未変更ファイルのスキップと新規行のみのパースで効率的に動作する。Claude Code でセッションが進行し JSONL に書き込まれるたびに、inotify が検知して数秒以内に差分が取り込まれる。

## よく使うコマンド

| コマンド | 説明 |
|---|---|
| `make` | ターゲット一覧を表示（= `make help`） |
| `make check` | fmt + clippy + test を一括実行 |
| `make clippy` | リント（warnings = error） |
| `cargo test -p otel-cc` | otel-cc のテストのみ実行 |
| `make init-firewall` | Dev Container ファイアウォール初期化（初回のみ） |
