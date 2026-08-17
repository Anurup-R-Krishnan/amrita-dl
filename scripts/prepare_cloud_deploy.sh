#!/usr/bin/env bash
set -e

echo "🚀 Preparing Amrita Exam Papers Cloud Deployment Bundle..."

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"

# 1. Sanity Checks
if [ ! -f "${ROOT_DIR}/index.db" ]; then
    echo "❌ Error: index.db not found in ${ROOT_DIR}!"
    exit 1
fi

if [ ! -d "${ROOT_DIR}/amrita-exam-papers-indexed" ]; then
    echo "❌ Error: amrita-exam-papers-indexed directory not found!"
    exit 1
fi

PDF_COUNT=$(find "${ROOT_DIR}/amrita-exam-papers-indexed" -type f -name "*.pdf" | wc -l)
echo "📄 Verified ${PDF_COUNT} indexed PDF papers."

# 2. Build Release Binary
echo "🔨 Building optimized Rust release binary..."
cd "${ROOT_DIR}"
cargo build --release --bin server

# 3. Create dist directory
echo "📦 Packaging deployment bundle into ${DIST_DIR}..."
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
echo "📂 Copying index database and indexed PDFs to bundle..."
cp "${ROOT_DIR}/index.db" "${DIST_DIR}/data/index.db"
cp -r "${ROOT_DIR}/amrita-exam-papers-indexed" "${DIST_DIR}/data/amrita-exam-papers-indexed"

echo "✅ Production cloud deployment bundle successfully prepped at ${DIST_DIR}!"
echo "💡 Raw directory 'amrita-exam-papers' was automatically excluded (Saved ~50% disk space)."
