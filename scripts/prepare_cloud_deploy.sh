#!/usr/bin/env bash
set -euo pipefail

echo "[BUILD] Preparing Amrita Exam Papers Cloud Deployment Bundle..."

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"

INDEXED_SRC="${INDEXED_ROOT:-${ROOT_DIR}/amrita-exam-papers-indexed}"
if [ ! -d "${INDEXED_SRC}" ] && [ -d "/run/media/anuruprkris/DATA/amrita-exam-papers-indexed" ]; then
    INDEXED_SRC="/run/media/anuruprkris/DATA/amrita-exam-papers-indexed"
fi

DB_PATH="${INDEX_DB:-${INDEXED_SRC}/index.db}"
if [ ! -f "${DB_PATH}" ] && [ -f "${ROOT_DIR}/index.db" ]; then
    DB_PATH="${ROOT_DIR}/index.db"
fi

# 1. Sanity Checks
if [ ! -f "${DB_PATH}" ]; then
    echo "[ERROR] index.db not found at ${DB_PATH}!"
    exit 1
fi

if [ ! -d "${INDEXED_SRC}" ]; then
    echo "[ERROR] indexed directory not found at ${INDEXED_SRC}!"
    exit 1
fi

PDF_COUNT=$(find "${INDEXED_SRC}" -type f -name "*.pdf" | wc -l)
echo "[INFO] Verified ${PDF_COUNT} indexed PDF papers at ${INDEXED_SRC}."

# 2. Build Release Binary
echo "[BUILD] Building optimized Rust release binary..."
cd "${ROOT_DIR}"
cargo build --release --bin server

# 3. Create dist directory
echo "[PACKAGING] Packaging deployment bundle into ${DIST_DIR}..."
rm -rf "${DIST_DIR}"
mkdir -p "${DIST_DIR}/data"

# Copy Binary & Public Web Static Assets
cp "${ROOT_DIR}/target/release/server" "${DIST_DIR}/server"
chmod +x "${DIST_DIR}/server"

if [ -d "${ROOT_DIR}/web" ]; then
    cp -r "${ROOT_DIR}/web" "${DIST_DIR}/web"
fi

# Copy Cloud Deployment Configurations (if present)
[ -f "${ROOT_DIR}/Dockerfile" ] && cp "${ROOT_DIR}/Dockerfile" "${DIST_DIR}/Dockerfile"
[ -f "${ROOT_DIR}/.dockerignore" ] && cp "${ROOT_DIR}/.dockerignore" "${DIST_DIR}/.dockerignore"
[ -f "${ROOT_DIR}/docker-compose.yml" ] && cp "${ROOT_DIR}/docker-compose.yml" "${DIST_DIR}/docker-compose.yml"
[ -f "${ROOT_DIR}/scripts/amrita-server.service" ] && cp "${ROOT_DIR}/scripts/amrita-server.service" "${DIST_DIR}/amrita-server.service"

# Link / Copy Index Database and Indexed Papers
echo "[COPY] Copying index database (${DB_PATH}) to bundle..."
cp "${DB_PATH}" "${DIST_DIR}/data/index.db"

SKIP_PDF_COPY="${SKIP_PDF_COPY:-0}"
if [ "${SKIP_PDF_COPY}" -eq 1 ]; then
    echo "[INFO] SKIP_PDF_COPY=1: Skipping PDF file copy (using Object Storage / CDN redirect mode)."
else
    echo "[COPY] Copying indexed PDF files into bundle..."
    cp -r "${INDEXED_SRC}" "${DIST_DIR}/data/amrita-exam-papers-indexed"
fi

echo "[SUCCESS] Production cloud deployment bundle successfully prepped at ${DIST_DIR}!"
