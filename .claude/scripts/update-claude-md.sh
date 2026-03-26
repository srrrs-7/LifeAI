#!/usr/bin/env bash
set -euo pipefail

# CLAUDE.md Auto-Updater
#
# コミット後にコードベースの変更を検知し、claude -p で CLAUDE.md を自動更新する。
# context-hub-runner.sh と同様の設計：24h 以内の再実行は自動スキップ。
#
# 処理フロー:
#   1. 実行間隔チェック — 前回から 24h 未満ならスキップ（--force で無視可能）
#   2. 直近コミットで .rs / .toml ファイルが変更されたか確認
#   3. 変更があれば claude -p --dangerously-skip-permissions で CLAUDE.md を更新
#   4. タイムスタンプを記録して次回スキップ判定に使用
#
# 呼び出し元: post-commit Git Hook（バックグラウンド実行）
#
# Usage:
#   ./update-claude-md.sh [--force]
#
# Options:
#   --force    24h スキップチェックを無視して強制実行

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(git -C "${SCRIPT_DIR}" rev-parse --show-toplevel 2>/dev/null || true)"
PROJECT_ROOT="${PROJECT_ROOT:-$(cd "${SCRIPT_DIR}/../.." && pwd)}"
LAST_RUN_FILE="${SCRIPT_DIR}/.claude-md-last-run"
BACKUP_FILE="${SCRIPT_DIR}/.claude-md-backup"
FORCE=false

# ── 引数解析 ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case $1 in
        --force) FORCE=true; shift ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

# ── 実行間隔チェック（24h スキップ）──────────────────────────────────
if [[ "${FORCE}" == "false" && -f "${LAST_RUN_FILE}" ]]; then
    last_run=$(cat "${LAST_RUN_FILE}")
    now=$(date +%s)
    elapsed=$(( now - last_run ))
    if [[ ${elapsed} -lt 86400 ]]; then
        hours_remaining=$(( (86400 - elapsed) / 3600 ))
        echo "[update-claude-md] Skipping: last run was $(( elapsed / 3600 ))h ago" \
             "(next in ~${hours_remaining}h). Use --force to override."
        exit 0
    fi
fi

# ── Rust / TOML ファイルの変更チェック ───────────────────────────────
changed_files=""
if git -C "${PROJECT_ROOT}" rev-parse HEAD~1 &>/dev/null; then
    changed_files=$(
        git -C "${PROJECT_ROOT}" diff --name-only HEAD~1 HEAD 2>/dev/null \
            | grep -E '\.(rs|toml)$' \
            | grep -vE '^CLAUDE\.md$' \
        || true
    )
else
    # 初回コミット — 常に更新
    changed_files="(initial commit)"
fi

if [[ -z "${changed_files}" ]]; then
    echo "[update-claude-md] No Rust/TOML files changed. Skipping CLAUDE.md update."
    exit 0
fi

changed_summary="$(echo "${changed_files}" | head -3 | tr '\n' ' ')"
echo "[update-claude-md] Changes detected: ${changed_summary}..."

# ── claude CLI の存在確認 ─────────────────────────────────────────────
if ! command -v claude &>/dev/null; then
    echo "[update-claude-md] claude CLI not found. Skipping."
    exit 0
fi

CLAUDE_MD="${PROJECT_ROOT}/CLAUDE.md"

# 更新前のバックアップ
if [[ -f "${CLAUDE_MD}" ]]; then
    cp "${CLAUDE_MD}" "${BACKUP_FILE}"
fi

# ── /init 相当のプロンプトで CLAUDE.md を更新 ─────────────────────────
PROMPT="$(cat <<'PROMPT_EOF'
コードベースを分析して CLAUDE.md を最新の状態に更新してください。

## 更新方針
- 既存の CLAUDE.md の内容を確認し、変更が必要な箇所のみ更新する
- よく使うコマンド（ビルド・テスト・リント・フォーマット等）が正確か確認
- アーキテクチャの概要（モジュール構成・設計パターン・データフロー）を現在のコードに合わせる
- 既存の正確な情報はそのまま保持し、古くなった記述のみ修正する

## 守ること
- ファイル先頭の以下のテキストは必ず保持すること:
  "# CLAUDE.md\n\nThis file provides guidance to Claude Code (claude.ai/code) when working with code in this repository."
- 不要な情報の追加や内容の大幅な変更は避ける
- CLAUDE.md ファイルに直接書き込むこと
PROMPT_EOF
)"

echo "[update-claude-md] Updating CLAUDE.md via claude -p..."
cd "${PROJECT_ROOT}"

if claude -p --dangerously-skip-permissions "${PROMPT}" > /dev/null 2>&1; then
    date +%s > "${LAST_RUN_FILE}"
    echo "[update-claude-md] CLAUDE.md updated successfully."
else
    echo "[update-claude-md] claude -p failed. Restoring backup." >&2
    [[ -f "${BACKUP_FILE}" ]] && cp "${BACKUP_FILE}" "${CLAUDE_MD}"
    exit 1
fi
