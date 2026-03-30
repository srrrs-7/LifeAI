# LifeAI

## 解決する課題

**Claude Code を日常的に使う個人開発者**が抱える2つの課題に対応する。

### 1. 活動の記録が残らない

コードを書く・調べる・考える — 日々の知的作業は形に残りにくい。日報を書こうにも「今日何をしたか」を思い出すところから始まり、続かない。アイデアは思いついた瞬間が一番鮮度が高いのに、構造化する前に流れてしまう。

**LifeAI のスキルシステム**は、対話形式のヒアリングで記憶を引き出し、構造化された成果物（日報・アイデアシート・ブログ記事）に変換する。テンプレートに沿って書くのではなく、会話の中から自然に情報を抽出する。

| スキル | 呼び出し | 解決する課題 | 成果物 |
|---|---|---|---|
| `daily-report` | `/daily-report` | 日報を書く負荷が高く、続かない | Markdown + SVG インフォグラフィック |
| `idea` | `/idea` | アイデアが構造化される前に忘れる | 10問ヒアリング → アイデアシート |
| `blog` | `/blog` | 技術記事を書き始められない | ヒアリング → ブログ記事 |
| `commit` | `/commit` | コミットメッセージを考える手間を省く | Conventional Commits 形式の自動コミット |
| `grafana-report` | `/grafana-report` | Claude Code 使用状況の定量把握が面倒 | KPI 分析・改善提案の Markdown レポート |
| `gen-skill` | `/gen-skill` | 新しいスキルの雛形を手で作るのが面倒 | スキル一式のスキャフォールド |

### 2. Claude Code の使い方が効率的かどうかわからない

Claude Code は従量課金で、使い方次第でコストが大きく変わる。しかし「今月いくら使ったか」「どのプロジェクトが高コストか」「キャッシュは効いているか」を確認する手段が標準では提供されていない。非効率な使い方（セッションを毎回新規作成する、冗長な応答を放置する、ツールエラーを繰り返す）に気づけないまま出費が膨らむ。

**otel-cc**（`core/otel-cc/`）は、Claude Code のセッションログを自動解析し、使用状況を Grafana ダッシュボードで可視化する。設定不要で動き、ブラウザを開けば今の状態がわかる。

#### otel-cc が答える問い

| 問い | 対応するダッシュボード | 確認できること |
|---|---|---|
| 今月いくら使った？ | Cost Management | 日別コスト推移、24h移動平均、予算消化率 |
| どのプロジェクトがコスト高い？ | Cost Management | プロジェクト別コスト内訳・比較 |
| セッションの使い方は効率的？ | Session Efficiency | セッションあたりコスト、キャッシュヒット率、圧縮イベント頻度 |
| どのツールで無駄が出ている？ | Tool Analytics | ツール別エラー率ランキング、アンチパターン検出 |
| モデル選択は適切？ | Model Optimization | モデル別コスト効率、トークン効率比較 |
| 異常な使い方が発生していない？ | Anomaly Detection | コスト・エラー率・キャッシュの ±2σ 逸脱検知 |
| 先週と比べてどう変わった？ | Periodic Review | 期間比較、7日ローリング平均、累積トレンド |

#### 効率の目安

| 指標 | 良好 | 要注意 | 問題 |
|---|---|---|---|
| セッションあたりコスト | < $8 | $8-$15 | > $15 |
| キャッシュヒット率 | >= 95% | 80-95% | < 80% |
| 出力/入力トークン比 | < 5 | 5-10 | > 10 |
| ツールエラー率 | < 5% | 5-10% | > 10% |
| 圧縮イベント/セッション | < 0.2 | 0.2-0.5 | > 0.5 |

**よくある改善アクション:**
- キャッシュヒット率が低い → `claude --resume` / `--continue` でセッションを継続する
- 圧縮イベントが多い → タスクを分割して1セッションを短く保つ
- 特定ツールのエラー率が高い → 2-3回失敗したら `Agent(Explore)` に委ねる
- 出力/入力比が高い → プロンプトに簡潔さの制約を追加する

## セットアップ

**前提:** Docker, Docker Compose, VS Code + Dev Containers 拡張

```bash
# Dev Container を起動後
make hooks    # Git hooks インストール
make build    # ビルド確認
```

### otel-cc モニタリングの起動

```bash
# ホスト側で実行
docker compose -f .devcontainer/compose.yaml up otel-cc prometheus grafana
```

| URL | 用途 |
|---|---|
| `http://localhost:3000` | Grafana（7つのダッシュボードが自動プロビジョニング） |
| `http://localhost:9091/metrics` | Prometheus メトリクス直接確認 |
| `http://localhost:9091/api/stats` | JSON 統計 API（`?period=7&project=名前&user=名前`） |
| `http://localhost:9091/api/analytics` | ツール使用・セッション効率分析 |
| `http://localhost:9091/api/optimization` | コスト最適化提案 |
| `http://localhost:9091/api/benchmarks` | ベンチマーク比較データ |

設定不要で `~/.claude/projects/` のローカルログを自動解析する。

#### 月次予算の設定

Cost Management ダッシュボードの上部にある **"Monthly Budget ($)"** テキストボックスで月次予算を設定できる（デフォルト: $100）。Budget Remaining（残額）と Budget Consumption（消化率）が自動計算される。

#### OTel リアルタイム受信（オプション）

OTel リアルタイム受信を追加で有効にする場合は `~/.claude/settings.json` に以下を追記:

```json
{
  "env": {
    "CLAUDE_CODE_ENABLE_TELEMETRY": "1",
    "OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:4318",
    "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json"
  }
}
```

### チームモニタリング

チームで Prometheus/Grafana を共有し、各メンバーの Claude Code 使用状況を一元可視化できる。

#### サーバー側のセットアップ

```bash
# チーム用サーバーで実行
docker compose -f core/otel-cc/infra/docker-compose.team.yaml up -d
```

| URL | 用途 |
|---|---|
| `http://<server>:3000` | Grafana（チーム全体 + 個人別ダッシュボード） |
| `http://<server>:4318` | OTLP 受信エンドポイント |

#### チームメンバーの設定

各メンバーが `~/.claude/settings.json` に以下を追記:

```json
{
  "env": {
    "CLAUDE_CODE_ENABLE_TELEMETRY": "1",
    "OTEL_EXPORTER_OTLP_ENDPOINT": "http://<server-ip>:4318",
    "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json",
    "OTEL_RESOURCE_ATTRIBUTES": "user.name=<your-name>"
  }
}
```

`user.name` はチーム内で一意になるようにする。Grafana の `$user` 変数でフィルタリングすることで、個人の統計とチーム全体の統計を切り替えられる。

#### ローカルモニタリングでのユーザー名設定

ローカルで otel-cc を使う場合、環境変数 `OTEL_CC_USER` でユーザー名を設定できる（デフォルト: OS ユーザー名）。

### ログ収集の仕組み

3段構えでログを収集し、取りこぼしを防ぐ:

| トリガー | タイミング | 役割 |
|---|---|---|
| 起動時フルスキャン | バイナリ起動直後 | 初回データ取り込み |
| inotify ファイル監視 | JSONL 変更検知 → 2秒デバウンス後 | リアルタイム収集 |
| 定期スキャン | 60秒間隔 | inotify 取りこぼしのフォールバック |

各スキャンでは mtime + 行オフセットによる差分処理で、未変更ファイルのスキップと新規行のみのパースを行う。

## よく使うコマンド

| コマンド | 説明 |
|---|---|
| `make` | ターゲット一覧を表示 |
| `make check` | fmt + clippy + test を一括実行 |
| `cargo test -p otel-cc` | otel-cc のテストのみ実行 |
| `make coverage` | カバレッジ計測 |
| `make rebuild-otel-cc` | otel-cc を再ビルド+再起動（ホスト側） |
| `make logs-otel-cc` | otel-cc のログ確認（ホスト側） |
