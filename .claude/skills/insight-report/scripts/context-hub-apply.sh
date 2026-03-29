#!/usr/bin/env bash
set -euo pipefail

# Context Hub — Apply Proposed Patch
#
# context-hub-runner.sh が生成した unified diff パッチを CLAUDE.md に適用する。
# 適用前にプレビュー表示し、ユーザー確認を求める安全設計。
#
# Usage:
#   ./context-hub-apply.sh [--yes] [--date YYYY-MM-DD]
#
# Options:
#   --yes          確認プロンプトをスキップ
#   --date DATE    適用する日付のパッチを指定（デフォルト: 最新）

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
ASSETS_DIR="${SCRIPT_DIR}/../assets/context-hub"
AUTO_YES=false
TARGET_DATE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --yes)
            AUTO_YES=true
            shift
            ;;
        --date)
            TARGET_DATE="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# Find the latest patch file
if [[ -n "${TARGET_DATE}" ]]; then
    patch_file="${ASSETS_DIR}/${TARGET_DATE}/proposed.patch"
else
    # Find the most recent date directory with a patch
    patch_file=""
    for dir in $(ls -dr "${ASSETS_DIR}"/20* 2>/dev/null); do
        if [[ -f "${dir}/proposed.patch" ]]; then
            patch_file="${dir}/proposed.patch"
            break
        fi
    done
fi

if [[ -z "${patch_file}" || ! -f "${patch_file}" ]]; then
    echo "[context-hub-apply] No patch file found."
    echo "[context-hub-apply] Run context-hub-runner.sh first, or specify --date."
    exit 1
fi

echo "[context-hub-apply] Patch file: ${patch_file}"
echo ""

# Preview changes
echo "=== Patch Preview ==="
if git -C "${PROJECT_ROOT}" apply --stat "${patch_file}" 2>/dev/null; then
    echo ""
    echo "=== Diff Content ==="
    cat "${patch_file}"
    echo ""
else
    echo "[context-hub-apply] Warning: git apply --stat failed. Showing raw patch:"
    echo ""
    cat "${patch_file}"
    echo ""
fi

# Confirm
if [[ "${AUTO_YES}" == "false" ]]; then
    read -rp "[context-hub-apply] Apply this patch to CLAUDE.md? [y/N] " answer
    if [[ "${answer}" != "y" && "${answer}" != "Y" ]]; then
        echo "[context-hub-apply] Aborted."
        exit 0
    fi
fi

# Apply
cd "${PROJECT_ROOT}"
if git apply "${patch_file}"; then
    echo "[context-hub-apply] Patch applied successfully."
    echo "[context-hub-apply] Review changes with: git diff CLAUDE.md"
else
    echo "[context-hub-apply] Failed to apply patch cleanly."
    echo "[context-hub-apply] You may need to apply manually. Patch file: ${patch_file}"
    exit 1
fi
