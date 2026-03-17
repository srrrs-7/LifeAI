---
name: insight-report
description: Claudeの使用状況ログを解析し、トークン使用量・ツール利用頻度・エラーパターン等を調査。改善案を提示した上で修正対応まで行う。Use when analyzing Claude Code usage, optimizing workflow, or reducing token costs.
disable-model-invocation: true
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent
---

# Insight Report — Claude使用状況分析 & 改善

ultrathink

## Objective

Claudeの全セッションログを解析し、使用パターンの問題点を特定して、改善案の提示と修正対応を行う。

## Steps

### Step 1: 並列データ収集

以下の3つのサブエージェントを **並列（parallel）** で起動する。必ず1つのメッセージで複数のAgent tool callを同時に発行すること。

#### Agent A: ログ解析
- `python3 ${CLAUDE_SKILL_DIR}/scripts/analyze-logs.py` を実行してJSONレポートを取得
- 結果を解析し、以下を抽出:
  - トークン使用量の内訳（input / output / cache_creation / cache_read）
  - キャッシュ効率（cache_hit_rate, cache_usage_rate）
  - ツール使用頻度ランキング
  - エラー・失敗パターン
  - モデル使用状況
  - セッション数と平均メッセージ数

#### Agent B: 設定・スキル監査
- 以下のファイル/ディレクトリを調査:
  - `CLAUDE.md` — プロジェクト設定の有無と内容
  - `.claude/settings.local.json` — パーミッション設定
  - `~/.claude/settings.json` — グローバル設定
  - `.claude/skills/` — 登録済みスキルの一覧と各descriptionの品質
  - `.claude/agents/` — カスタムエージェントの有無
  - `~/.claude/projects/*/memory/` — メモリファイルの有無と内容
- 各設定の最適化ポイントを特定

#### Agent C: 会話パターン分析
- セッションログのJSONLファイルを直接読み込み:
  - ユーザーの典型的なリクエストパターンを分類
  - Claudeの応答品質（ツール使用の適切さ、冗長性）を評価
  - 繰り返し発生している問題パターンを特定
  - パーミッション拒否の頻度と対象ツールを調査

### Step 2: 統合レポート作成

3つのエージェントの結果を統合し、以下の形式でレポートを出力する:

```
## 📊 Claude使用状況レポート

### サマリー
- 総セッション数: X
- 総メッセージ数: X
- 総トークン使用量: input X / output X
- キャッシュ効率: X%

### 🔍 検出された課題
1. [優先度: 高/中/低] 課題の説明
   - 根拠: データから得られた具体的な数値
   - 影響: どの程度のコスト/効率への影響があるか

### 💡 改善案
| # | 課題 | 改善案 | 期待効果 | 対応方法 |
|---|------|--------|----------|----------|
| 1 | ... | ... | ... | 自動修正 / 手動対応 |

### 🛠 自動修正可能な項目
- [ ] 項目1の説明
- [ ] 項目2の説明
```

### Step 3: 修正対応

レポート出力後、ユーザーに確認を取る:

> **上記の改善案のうち、自動修正可能な項目を実行しますか？**
> - すべて実行
> - 個別に選択
> - 今回はスキップ

ユーザーの承認を得てから、以下の修正を実施:

- **CLAUDE.md の最適化**: 不足している設定の追加、冗長な記述の整理
- **settings.json の更新**: 頻繁に承認しているパーミッションの自動追加
- **スキルの改善**: description の改善、不要スキルの報告
- **メモリの整理**: 重複や古いメモリの報告、不足メモリの提案

## References

- [improvement-patterns.md](references/improvement-patterns.md) — 改善パターンのリファレンス。検出した課題をこのパターンと照合して改善案を導出すること。

## Important Notes

- ログファイルのパス: `~/.claude/projects/<project-hash>/*.jsonl`
- セッション情報: `~/.claude/sessions/*.json`
- ユーザーの承認なしにファイルを修正しないこと
- レポートは日本語で出力すること
