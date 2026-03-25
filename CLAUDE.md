# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Purpose

LifeAI は、個人の活動のレコード（日報・振り返り・気づき等）を残していくための AI 対話フレームワークです。Claude Code のスキルシステムを活用し、対話を通じて個人のライフマネジメントを支援します。Rust ベースのコアと Claude Code スキル群で構成されています。

## Commands

- `make build` — ビルド
- `make test` — テスト実行
- `make fmt` — フォーマット
- `make clippy` — リント（`-D warnings` 付き、警告はすべてエラー扱い）
- `make check` — fmt + clippy + test をまとめて実行
- `make hooks` — Git hooks インストール（`.githooks/` を使用）
- `make init-firewall` — Dev Container 用ファイアウォール初期化（要 root）
- `cargo test <test_name>` — 単一テスト実行

> **Note**: Rust コードは未実装のため、現時点では `cargo` 系コマンド（build/test/fmt/clippy）は Cargo.toml 追加後に利用可能になる。

## Architecture

### 全体構造

現時点ではプロジェクトの主要機能は Claude Code スキル群で構成されている。`core/` には将来の Rust コア機能のためのディレクトリ（`certifications/`, `cooking/` 等のドメイン別サブディレクトリ）が予約されているが、Rust コードはまだ存在しない。

### スキル・エージェント設計パターン

すべてのスキルは共通の「対話 → 委譲」パターンに従う：

1. **スキル（`.claude/skills/<name>/SKILL.md`）** — ユーザーとの対話型ヒアリングを担当（インライン実行）
2. **サブエージェント（`.claude/agents/<name>-writer.md`）** — ヒアリング結果をもとに成果物を生成（model: opus）
3. スキルは Agent ツールでサブエージェントに委譲し、結果をユーザーに返す

各スキルは `templates/` にテンプレート、`references/` にリファレンス資料、`assets/` に成果物を格納する。

### スキル一覧

- `daily-report/` — 対話型日報作成（テキスト + SVG インフォグラフィック）。成果物: `assets/<yyyy-mm-dd>/`
- `idea/` — アイデアブレスト＆構造化（10問ヒアリング → アイデアシート）。成果物: `assets/<theme-name>/`
- `insight-report/` — Claude 使用状況ログ解析 & ワークフロー改善 & プロジェクト知識自動抽出（並列サブエージェント4本で分析）
- `gen-skill/` — 対話型スキルスキャフォールド生成。新スキル作成時はこれを使う

### 設計方針

- **コンテキスト境界**: 対話ログや insights の集約時に PII 漏洩・コンテキスト汚染を防ぐため、プロジェクト単位でのコンテキスト境界を明確に設ける
- **insights 自動生成**: Git hooks でコードベース上に対話ログを収集し、CI/CD で insights を生成・ドキュメントに自動反映する方針（ドッグフーディングとして本プロジェクトで先行運用中）

### Dev Container

`.devcontainer/` で開発環境を定義。ファイアウォールによるネットワーク制限あり（`whitelist_domains.conf` で許可ドメインを管理）。

## Git Hooks

`.githooks/` に設定済み（`make hooks` でインストール）:
- **pre-commit**: `cargo fmt` + `cargo clippy --all-targets --all-features -- -D warnings`
- **pre-push**: `cargo test`
- **post-commit**: Context Hub による知識抽出をバックグラウンド実行（24h以内の再実行は自動スキップ）

## Coding Conventions

- Rust 命名規約に従う（snake_case for functions/variables, CamelCase for types）
- clippy warnings はすべて解消すること

## Claude Code Instructions

- 応答は日本語で行うこと
- 既存ファイルの修正には Edit ツールを優先し、Write は新規作成時のみ使用
- ファイル検索には Grep / Glob を使用し、Bash で grep/find/cat を直接実行しない
- コードを読む際は Read ツールを使用し、Bash で cat/head/tail を使わない
- 簡潔に応答し、不要な要約や繰り返しを避ける
- スキルやタスクの作成は対話形式で進める（自動生成禁止）
- スキルやプロンプトの設計時は、ゴール・方針・方向性の定義を最優先で明確化すること
