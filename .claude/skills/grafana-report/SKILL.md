---
name: grafana-report
description: otel-cc の /api/stats API からリアルタイム KPI を取得し、閾値判定と改善提案を Markdown レポート出力。コスト・キャッシュ効率・ツールエラー率・セッション効率を数値評価する（JSONL ログの深掘りには insight-report を使う）。Use when reviewing real-time Claude Code KPI via otel-cc API, checking daily/weekly numerical stats, or generating threshold-based reports.
argument-hint: "[days]"
disable-model-invocation: true
---

# Grafana Report — Claude Code 使用状況分析レポート

ultrathink

## Objective

otel-cc の `/api/stats` API から取得した統計データを分析し、Claude Code の使い方に関する KPI 評価・トレンド分析・改善提案をターミナル上に Markdown レポートとして出力する。

## Steps

### Step 1: 引数パースとデータ取得

1. `<user-argument>` から期間（日数）を取得する。未指定の場合はデフォルト 3 日とする
2. Bash ツールで以下を実行してデータを取得する:
   ```
   curl -s "http://localhost:9091/api/stats?period=<期間>"
   ```

### Step 2: データ検証

取得した JSON データを確認する。

**データ取得に失敗した場合:**
以下のエラーメッセージを出力して終了する:

```
## otel-cc 接続エラー

統計データを取得できませんでした。以下を確認してください:

1. otel-cc コンテナが起動しているか: `make logs-otel-cc`（ホスト側で実行）
2. コンテナの再起動: `make restart-infra`（ホスト側で実行）
3. ポート 9091 がアクセス可能か: `curl http://localhost:9091/health`
```

### Step 3: 派生メトリクス算出

取得した JSON データから以下の派生メトリクスを計算する:

| メトリクス | 算出式 |
|---|---|
| セッションあたりコスト | `total_cost_usd / total_sessions` |
| キャッシュヒット率 | `total_cache_read_tokens / (total_input_tokens + total_cache_read_tokens)` |
| 出力/入力トークン比 | `total_output_tokens / total_input_tokens` |
| ツールエラー率 | `total_tool_errors / total_tool_calls` |
| 圧縮イベント/セッション | `total_compression_events / total_sessions` |
| ツール呼び出しあたりコスト | `total_cost_usd / total_tool_calls` |

ゼロ除算が発生する場合は `N/A` と表示し、該当メトリクスの評価をスキップする。

### Step 4: 閾値判定

[閾値リファレンス](references/thresholds.md) を参照し、各メトリクスを Good / Warning / Problem の3段階で評価する。

ステータス表示:
- Good: 緑チェック記号
- Warning: 黄色警告記号
- Problem: 赤丸記号

### Step 5: 日次トレンド分析

`daily` 配列のデータから以下を分析する:

- **コストの急増**: 前日比 2 倍以上のコスト増加があった日を特定
- **エラー率スパイク**: 日次ツールエラー率が 10% を超えた日を特定
- **セッション数の異常**: 平均の 2 倍以上のセッション数があった日を特定
- **全体傾向**: 期間全体でコスト/効率が改善傾向か悪化傾向かを判定

### Step 6: プロジェクト別分析

`projects` 配列から以下を特定する:

- コストが最も高いプロジェクト（上位 3 件）
- セッションあたりコストが最も高いプロジェクト
- ツール呼び出しが最も多いプロジェクト

### Step 7: ユーザー別分析（複数ユーザーの場合）

`users` 配列に複数ユーザーが存在する場合、ユーザー間の比較を行う:

- コスト効率の比較
- キャッシュ効率の比較

単一ユーザーの場合はこのセクションをスキップする。

### Step 8: レポート出力

以下の構造で Markdown レポートをターミナルに出力する。ファイルには書き出さない。

```markdown
## Claude Code 使用状況レポート

**期間:** YYYY-MM-DD 〜 YYYY-MM-DD（N 日間）
**データ生成:** YYYY-MM-DDTHH:MM:SS
**総セッション数:** X | **総コスト:** $X.XX

---

### KPI スコアカード

| メトリクス | 値 | 状態 | 基準 |
|---|---|---|---|
| セッションあたりコスト | $X.XX | ステータス | < $8 |
| キャッシュヒット率 | XX.X% | ステータス | >= 95% |
| 出力/入力トークン比 | X.XX | ステータス | < 5 |
| ツールエラー率 | X.X% | ステータス | < 5% |
| 圧縮/セッション | X.XX | ステータス | < 0.2 |
| コスト/ツール呼び出し | $X.XXX | ステータス | < $0.05 |

### 日次トレンド

（日次データのサマリーテーブルと、検出された異常値の説明）

### プロジェクト別内訳

（上位プロジェクトのコスト・セッション数・効率指標テーブル）

### 改善提案

| 優先度 | 課題 | 提案 | 期待効果 |
|---|---|---|---|
| 高 | ... | ... | ... |

**具体的なアクション:**

1. （最も優先度の高い改善アクション）
2. ...
```

### Step 9: 改善提案の導出

各 Warning / Problem 判定のメトリクスに対して、以下の改善パターンを適用する:

- **セッションあたりコストが高い**: `claude --resume` でセッションを継続し、キャッシュを再利用する。タスクを小さく分割する。事前に Plan エージェントで設計を固める
- **キャッシュヒット率が低い**: 新規セッションの開始を減らし、`--resume` / `--continue` で前セッションを継続する
- **出力/入力比が高い**: プロンプトに「簡潔に」制約を追加。サブエージェントへの出力フォーマット制約を設ける
- **ツールエラー率が高い**: ファイル探索で 2〜3 回失敗したら `Agent(Explore)` に委ねる。Grep/Glob の使い方を見直す
- **圧縮イベントが多い**: セッションが長すぎる。タスクを分割するか `--resume` で継続する
- **コスト/ツール呼び出しが高い**: 無駄なツール呼び出しを減らす。事前調査を Agent に委譲する

改善提案は影響度の大きい順に並べ、具体的な Claude Code コマンドやオプションを含めること。

## References

- [閾値リファレンス](references/thresholds.md) — メトリクスの閾値定義と解釈ガイド

## Important Notes

- レポートはターミナルに Markdown で出力する（ファイル書き出しはしない）
- レポートは日本語で出力する
- データが存在しない期間（セッション 0）の場合は「対象期間にデータがありません」と表示して終了する
- 絶対値よりもトレンド（前回比・傾向）を重視した分析を行う
