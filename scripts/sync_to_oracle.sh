#!/usr/bin/env bash
set -euo pipefail

echo "[SYNC] Oracle Cloud Object Storage Dataset Synchronizer"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

REMOTE="${RCLONE_REMOTE:-oracle-amrita-papers:oracle-amrita-bucket}"
INDEXED_SRC="${INDEXED_ROOT:-${ROOT_DIR}/amrita-exam-papers-indexed}"
if [ ! -d "${INDEXED_SRC}" ] && [ -d "/run/media/anuruprkris/DATA/amrita-exam-papers-indexed" ]; then
    INDEXED_SRC="/run/media/anuruprkris/DATA/amrita-exam-papers-indexed"
fi

if [ ! -d "${INDEXED_SRC}" ]; then
    echo "[ERROR] Indexed dataset directory not found at ${INDEXED_SRC}!"
    exit 1
fi

PDF_COUNT=$(find "${INDEXED_SRC}" -type f -name "*.pdf" | wc -l)
echo "[INFO] Source dataset: ${INDEXED_SRC} (${PDF_COUNT} PDF files)"
echo "[INFO] Destination target: ${REMOTE}"

DRY_RUN_FLAG=""
if [ "${1:-}" = "--dry-run" ]; then
    DRY_RUN_FLAG="--dry-run"
    echo "[INFO] Running in DRY-RUN mode. No files will be uploaded."
fi

echo "[SYNC] Starting rclone sync (excluding .meta sidecar files)..."
rclone sync "${INDEXED_SRC}" "${REMOTE}" \
    --exclude "*.meta" \
    --transfers=16 \
    --checkers=32 \
    --checksum \
    --fast-list \
    --progress \
    ${DRY_RUN_FLAG}

echo "[SUCCESS] Oracle Cloud Object Storage sync completed successfully!"
