#!/usr/bin/env bash
set -e

echo "[BUILD] Preparing Amrita Exam Papers Cloud Deployment Bundle..."

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"

INDEXED_SRC="${INDEXED_ROOT:-./amrita-exam-papers-indexed}"
if [ ! -d "${INDEXED_SRC}" ] && [ -d "/run/media/anuruprkris/DATA/amrita-exam-papers-indexed" ]; then
    INDEXED_SRC="/run/media/anuruprkris/DATA/amrita-exam-papers-indexed"
fi

# 1. Sanity Checks
if [ ! -f "${ROOT_DIR}/index.db" ]; then
    echo "[ERROR] Error: index.db not found in ${ROOT_DIR}!"
    exit 1
fi

if [ ! -d "${INDEXED_SRC}" ]; then
    echo "[ERROR] Error: indexed directory not found at ${INDEXED_SRC}!"
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

# Copy Cloud Deployment Configurations
cp "${ROOT_DIR}/Dockerfile" "${DIST_DIR}/Dockerfile"
cp "${ROOT_DIR}/.dockerignore" "${DIST_DIR}/.dockerignore"
cp "${ROOT_DIR}/docker-compose.yml" "${DIST_DIR}/docker-compose.yml"

# Link / Copy Index Database and Indexed Papers (Excluding raw folders)
echo "[COPY] Copying index database and indexed PDFs to bundle..."
cp "${ROOT_DIR}/index.db" "${DIST_DIR}/data/index.db"
cp -r "${INDEXED_SRC}" "${DIST_DIR}/data/amrita-exam-papers-indexed"

echo "[SUCCESS] Production cloud deployment bundle successfully prepped at ${DIST_DIR}!"
echo "[INFO] Raw directory 'amrita-exam-papers' was automatically excluded (Saved ~50% disk space)."
