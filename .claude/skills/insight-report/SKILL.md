---
name: insight-report
description: Claudeの使用状況ログを解析し、トークン使用量・ツール利用頻度・エラーパターン等を調査。対話ログからプロジェクト知識を抽出しCLAUDE.mdに反映。Use when analyzing Claude Code usage, optimizing workflow, reducing token costs, or extracting project knowledge from conversation logs.
disable-model-invocation: true
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent
---

# Insight Report — Claude使用状況分析 & 改善

ultrathink

## Objective

Claudeの全セッションログを解析し、使用パターンの問題点を特定して、改善案の提示と修正対応を行う。さらに、対話ログからプロジェクト固有の知識（技術知見・規約・意思決定等）を抽出し、CLAUDE.md に反映する。

## Steps

### Step 1: 並列データ収集

以下の4つのサブエージェントを **並列（parallel）** で起動する。必ず1つのメッセージで複数のAgent tool callを同時に発行すること。

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

#### Agent D: セマンティック知識抽出
- `python3 ${CLAUDE_SKILL_DIR}/scripts/extract-conversations.py` を実行して対話ペアを取得
- [semantic-extraction-guide.md](references/semantic-extraction-guide.md) を読み込み、抽出カテゴリと判定基準を確認
- 現在の `CLAUDE.md` を読み込み、既存の記載内容を把握
- 対話ペアから以下の5カテゴリの知識を抽出:
  1. **technical_insight**: ライブラリの使い方、API の振る舞い、環境固有の注意点
  2. **convention**: コーディングスタイル、命名規則、合意されたパターン
  3. **decision**: アーキテクチャ選択、技術選定の理由
  4. **workflow**: 繰り返し実行される手順、ベストプラクティス
  5. **known_issue**: 回避策付きの問題、ハマりポイント
- 各アイテムを以下の形式で報告:
  - カテゴリ / 要約（1行）/ 詳細（2-3行）/ ソースセッション / 信頼度（high/medium/low）/ CLAUDE.md 反映先セクション
- CLAUDE.md に既に記載されている内容と意味的に重複するものは除外
- PII（個人名、メールアドレス、API キー等）を含むものは除外

### Step 2: 統合レポート作成

4つのエージェントの結果を統合し、以下の形式でレポートを出力する:

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

### 📚 抽出されたプロジェクト知識（Agent D）

#### 新規 insights（CLAUDE.md 未反映）
| # | カテゴリ | 要約 | 信頼度 | 反映先セクション |
|---|---------|------|--------|----------------|
| 1 | ... | ... | high/medium/low | ... |

#### 既存と重複する insights（スキップ）
- ...（重複のため除外された項目をリスト）
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

### Step 4: プロジェクト知識の CLAUDE.md 反映

Agent D の抽出結果に基づき、CLAUDE.md への知識反映を提案する。

1. CLAUDE.md を Read で読み込み、現在の構造を把握
2. confidence が high の insights を対応セクションにマッピング:
   - `technical_insight` → Architecture セクション（または新規 Technical Notes セクション）
   - `convention` → Coding Conventions セクション
   - `decision` → Architecture セクション
   - `workflow` → Commands セクション（または新規 Workflows セクション）
   - `known_issue` → 新規 Known Issues セクション
3. ユーザーに反映内容のプレビューを提示:

> **CLAUDE.md に以下のプロジェクト知識を追加しますか？**
>
> **Coding Conventions に追加:**
> - 〇〇のパターンでは XX を使用する
>
> **Architecture に追加:**
> - 〇〇の理由で YY アーキテクチャを採用
>
> → すべて反映 / 個別に選択 / スキップ

4. ユーザー承認後、Edit ツールで CLAUDE.md の該当セクションに追記
5. 新規セクション（Technical Notes / Workflows / Known Issues）が必要な場合は、既存セクション構造に合わせて追加

## References

- [improvement-patterns.md](references/improvement-patterns.md) — 改善パターンのリファレンス。検出した課題をこのパターンと照合して改善案を導出すること。
- [semantic-extraction-guide.md](references/semantic-extraction-guide.md) — セマンティック知識抽出のカテゴリ定義・判定基準。Agent D が参照すること。

## Important Notes

- ログファイルのパス: `~/.claude/projects/<project-hash>/*.jsonl`
- セッション情報: `~/.claude/sessions/*.json`
- ユーザーの承認なしにファイルを修正しないこと
- レポートは日本語で出力すること
- バッチ実行: `scripts/context-hub-runner.sh` で非対話的に知識抽出のみを実行可能（提案ファイルを出力し、自動反映はしない）
