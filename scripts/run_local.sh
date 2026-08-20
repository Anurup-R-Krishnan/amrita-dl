#!/usr/bin/env bash
# Local dev server launcher.
# Requires: OCI bucket set to Public visibility for PDF redirects to work.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

BINARY="${ROOT_DIR}/target/release/server"
if [ ! -f "${BINARY}" ]; then
    echo "[ERROR] Release binary not found. Run: cargo build --release --bin server"
    exit 1
fi

export PORT="${PORT:-3000}"
export INDEX_DB="${INDEX_DB:-${ROOT_DIR}/index.db}"

# Namespace: axrtbfdmqkku  Bucket: Oracle-amrita-bucket  Region: ap-hyderabad-1
export STORAGE_PUBLIC_URL="${STORAGE_PUBLIC_URL:-https://objectstorage.ap-hyderabad-1.oraclecloud.com/n/axrtbfdmqkku/b/Oracle-amrita-bucket/o}"

# INDEXED_ROOT only needed for local disk fallback (not used when STORAGE_PUBLIC_URL is set)
export INDEXED_ROOT="${INDEXED_ROOT:-${ROOT_DIR}/amrita-exam-papers-indexed}"

export CORS_ALLOWED_ORIGINS="${CORS_ALLOWED_ORIGINS:-http://localhost:3000,http://127.0.0.1:3000,https://amritapapers.pages.dev,https://amritapapers-1x5.pages.dev}"

echo "[INFO] Starting amrita-dl server"
echo "[INFO] PORT              = ${PORT}"
echo "[INFO] INDEX_DB          = ${INDEX_DB}"
echo "[INFO] STORAGE_PUBLIC_URL= ${STORAGE_PUBLIC_URL}"
echo "[INFO] CORS origins      = ${CORS_ALLOWED_ORIGINS}"

exec "${BINARY}"
