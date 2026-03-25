#!/usr/bin/env bash
set -euo pipefail

# AI Context Hub — Batch Knowledge Extraction Runner
#
# 対話ログからセマンティック知識を非対話的に抽出し、CLAUDE.md への追記提案を生成する。
# 自動反映はせず、提案（proposed-changes.md）のみを出力する安全設計。
#
# 処理フロー:
#   1. 実行間隔チェック — 前回から24h未満ならスキップ（--force で無視可能）
#   2. extract-conversations.py で直近N日分の対話ログを JSON に変換
#   3. 抽出ガイド + 現在の CLAUDE.md + 対話ログからプロンプトを構築
#   4. Claude CLI（非対話モード）で知識抽出を実行し、提案ファイルに出力
#   5. タイムスタンプを記録して次回のスキップ判定に使用
#
# 呼び出し元: post-commit Git Hook（バックグラウンド実行）
# 出力先:     assets/context-hub/<yyyy-mm-dd>/proposed-changes.md
#
# Usage:
#   ./context-hub-runner.sh [--since DAYS] [--force]
#
# Options:
#   --since DAYS  直近N日分のログのみ処理（デフォルト: 7）
#   --force       最終実行チェックをスキップ

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
ASSETS_DIR="${SCRIPT_DIR}/../assets/context-hub"
LAST_RUN_FILE="${ASSETS_DIR}/.last-run"
SINCE_DAYS=7
FORCE=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --since)
            SINCE_DAYS="$2"
            shift 2
            ;;
        --force)
            FORCE=true
            shift
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# Check if 24h have passed since last run
if [[ "${FORCE}" == "false" && -f "${LAST_RUN_FILE}" ]]; then
    last_run=$(cat "${LAST_RUN_FILE}")
    now=$(date +%s)
    elapsed=$(( now - last_run ))
    if [[ ${elapsed} -lt 86400 ]]; then
        hours_remaining=$(( (86400 - elapsed) / 3600 ))
        echo "[context-hub] Skipping: last run was $(( elapsed / 3600 ))h ago (next run in ~${hours_remaining}h). Use --force to override."
        exit 0
    fi
fi

# Create output directory
today=$(date +%Y-%m-%d)
output_dir="${ASSETS_DIR}/${today}"
mkdir -p "${output_dir}"

echo "[context-hub] Extracting conversations from last ${SINCE_DAYS} days..."

# Step 1: Extract conversation pairs
conversations_file="${output_dir}/conversations.json"
python3 "${SCRIPT_DIR}/extract-conversations.py" --since "${SINCE_DAYS}" > "${conversations_file}"

total=$(python3 -c "import json,sys; d=json.load(open('${conversations_file}')); print(d.get('total_conversations', 0))")
echo "[context-hub] Found ${total} conversation pairs."

if [[ "${total}" -eq 0 ]]; then
    echo "[context-hub] No conversations found. Skipping knowledge extraction."
    rm -f "${conversations_file}"
    rmdir "${output_dir}" 2>/dev/null || true
    exit 0
fi

# Step 2: Generate knowledge extraction prompt
claude_md="${PROJECT_ROOT}/CLAUDE.md"
guide="${SCRIPT_DIR}/../references/semantic-extraction-guide.md"

prompt="以下の対話ログから、プロジェクト固有の知識を抽出してください。

## 抽出ガイド
$(cat "${guide}")

## 現在の CLAUDE.md
$(cat "${claude_md}")

## 対話ログ（JSON）
$(cat "${conversations_file}")

## 指示
1. 上記の対話ログを分析し、semantic-extraction-guide.md のカテゴリに従って知識を抽出してください
2. CLAUDE.md に既に記載されている内容と重複するものは除外してください
3. 以下のMarkdown形式で出力してください:

# Context Hub — 知識抽出レポート (${today})

## 抽出された知識

| # | カテゴリ | 要約 | 詳細 | 信頼度 | 反映先セクション |
|---|---------|------|------|--------|----------------|

## CLAUDE.md への追記提案

各セクションごとに追記すべき内容を具体的に記述してください。
"

# Step 3: Run Claude CLI for knowledge extraction
proposed_changes="${output_dir}/proposed-changes.md"
echo "[context-hub] Running knowledge extraction with Claude CLI..."

if command -v claude &> /dev/null; then
    echo "${prompt}" | claude -p --output-format text > "${proposed_changes}" 2>/dev/null
    echo "[context-hub] Proposed changes written to: ${proposed_changes}"
else
    echo "[context-hub] Claude CLI not found. Saving prompt for manual execution."
    echo "${prompt}" > "${output_dir}/extraction-prompt.md"
    echo "[context-hub] Prompt saved to: ${output_dir}/extraction-prompt.md"
    echo "[context-hub] Run manually: cat ${output_dir}/extraction-prompt.md | claude -p > ${proposed_changes}"
fi

# Update last run timestamp
date +%s > "${LAST_RUN_FILE}"

echo "[context-hub] Done. Review proposed changes at: ${output_dir}/"
