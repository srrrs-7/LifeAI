---
name: commit
description: Git commit (and optionally push) with auto-generated or specified message. Use when the user wants to git commit, push, コミット, プッシュ, save changes, or stage and commit.
argument-hint: "[commit message] [push]"
disable-model-invocation: true
---

# Git Commit Skill

変更を git commit する。コミットメッセージは引数で指定可能、省略時は diff から自動生成する。引数末尾に `push` を含めると push も実行する。

ultrathink

## Usage Examples

- `/commit` — diff から自動メッセージ生成してコミット
- `/commit fix: typo修正` — 指定メッセージでコミット
- `/commit push` — 自動メッセージでコミット + プッシュ
- `/commit fix: typo修正 push` — 指定メッセージでコミット + プッシュ

## Step 1: Parse Arguments

`$ARGUMENTS` を解析する:

1. 引数の末尾が単語 `push`（大文字小文字問わず）で終わるか判定し、`should_push` フラグを設定する
2. `push` を除いた残りの文字列をコミットメッセージ候補とする（trim 後に空ならメッセージ自動生成モード）

## Step 2: Check Working Tree Status

1. `git status --porcelain` を実行する
2. 変更がなければ「コミットする変更がありません」と報告して終了する
3. 変更がある場合、変更ファイル一覧を把握する

## Step 3: Review Diff

1. `git diff` でアンステージの差分を確認する
2. `git diff --staged` でステージ済みの差分を確認する
3. 差分の内容を理解し、変更の意図を把握する

## Step 4: Generate Commit Message (if needed)

コミットメッセージが引数で指定されていない場合:

1. `git log --oneline -5` で直近のコミットスタイルを確認する
2. diff の内容を分析し、Conventional Commits 形式でコミットメッセージを生成する:
   - `feat:` — 新機能
   - `fix:` — バグ修正
   - `docs:` — ドキュメント
   - `refactor:` — リファクタリング
   - `chore:` — 雑務
   - `test:` — テスト
   - `style:` — フォーマット
   - `perf:` — パフォーマンス改善
   - `ci:` — CI 設定
   - `revert:` — リバート
3. メッセージは簡潔に（日本語または英語、直近コミットのスタイルに合わせる）
4. 複数の変更がある場合は最も重要な変更を要約し、本文に詳細を記載する

## Step 5: Stage Files

1. 変更ファイルを個別に `git add <file>` でステージングする
2. **`git add -A` や `git add .` は絶対に使わない** — 必ず具体的なファイルパスを指定する
3. 以下の機密ファイルが含まれていないか確認し、含まれていれば警告してスキップする:
   - `.env`, `.env.*`
   - `credentials`, `secret`, `token` を含むファイル名
   - `*.pem`, `*.key`, `*.p12`
   - `id_rsa`, `id_ed25519` 等の SSH 鍵
4. 既にステージ済みのファイルがあればそれも含めて進める

## Step 6: Commit

1. コミットメッセージは HEREDOC 形式で渡す:
   ```bash
   git commit -F - <<'COMMIT_MSG'
   <type>: <subject>

   <body (optional)>

   Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
   COMMIT_MSG
   ```
2. メッセージ末尾には必ず `Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>` を付ける
3. **`--no-verify` は使わない** — pre-commit hook を必ず通す
4. **`--amend` は使わない** — 常に新規コミットを作成する
5. pre-commit hook が失敗した場合:
   a. エラー内容を確認する
   b. 問題を修正する（fmt, clippy 等）
   c. 修正を含めて再度ステージング + コミットする（amend ではなく新規コミット）

## Step 7: Push (if requested)

1. `should_push` が true の場合のみ `git push` を実行する
2. **`--force` は使わない**
3. push が失敗した場合はエラー内容を報告する

## Step 8: Report

結果を簡潔に日本語で報告する:

- コミットハッシュ（短縮形）
- コミットメッセージ
- 変更ファイル数
- push した場合はその結果

## Important Rules

- 応答は日本語で行う
- `git add -A` / `git add .` は使わない。具体的なファイルを指定する
- `--no-verify` / `--force` / `--amend` は使わない
- 機密ファイル（.env, credentials, 鍵ファイル等）をコミットしない
- コミットメッセージは HEREDOC で渡す
- Co-Authored-By トレイラーを必ず付ける
- pre-commit hook 失敗時は修正して新規コミットする
