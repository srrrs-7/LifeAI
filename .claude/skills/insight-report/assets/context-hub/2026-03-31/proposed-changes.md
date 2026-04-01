# Context Hub — 知識抽出レポート (2026-03-31)

## 抽出された知識

| # | カテゴリ | 要約 | 詳細 | 信頼度 | 反映先セクション |
|---|---------|------|------|--------|----------------|
| 1 | decision | commit スキルの push は明示的な引数指定時のみ実行する | `/commit` はコミットのみ、`/commit push` でコミット+プッシュ。安全側に倒す設計判断。副作用のあるスキルは `disable-model-invocation: true` で手動呼び出し限定にする。(session: 0192ed3a) | medium | スキル・エージェント設計パターン |
| 2 | convention | 副作用のあるスキルは `disable-model-invocation: true` を設定する | commit, deploy 等の外部状態を変更するスキルはユーザー手動呼び出しのみに制限する。gen-skill のヒアリングで一貫して適用されている。(session: 0192ed3a, ef13d4e6) | medium | スキル・エージェント設計パターン |
| 3 | workflow | スキル新規作成は `/gen-skill` → 対話ヒアリング → skill-writer サブエージェント委譲の流れ | commit スキル、grafana-report スキルともにこのフローで作成。ヒアリングは4ラウンド（Purpose → Behavior → Advanced → Confirm）で構造化されている。(session: 0192ed3a, c6410af1, ef13d4e6) | high | スキル新規追加フロー |

## CLAUDE.md への変更パッチ

対話ログの大部分は外部プロジェクト（Azure Blob/Bicep、React/TanStack Query、Dependabot）に関するもので、lifeai プロジェクト固有の知識は限定的でした。

抽出した3件について:
- **#1, #2**: スキルの設計パターンとして有用だが、既に `gen-skill/SKILL.md` のヒアリングフロー内（Invocation style mapping テーブル）に記載済み。CLAUDE.md への重複記載は不要。
- **#3**: 「スキル新規追加フロー」セクションに既に `/gen-skill` 使用の指示がある。

**変更提案なし**

現在の CLAUDE.md は対話ログから得られる知識を既にカバーしています。
