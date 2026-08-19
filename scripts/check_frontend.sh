#!/usr/bin/env bash
set -euo pipefail

echo "========================================="
echo "  Amrita Frontend Automated CI Check     "
echo "========================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
INDEX_FILE="${ROOT_DIR}/web/index.html"

if [ ! -f "$INDEX_FILE" ]; then
    echo "[ERROR] $INDEX_FILE does not exist!"
    exit 1
fi

echo "[OK] Checking index.html file existence..."

# Check 1: Ensure Pantone 7426C maroon color #A4123F is present
if grep -q "#A4123F" "$INDEX_FILE"; then
    echo "[OK] Brand color Pantone 7426C Maroon (#A4123F) verified."
else
    echo "[ERROR] Missing primary brand color #A4123F in $INDEX_FILE!"
    exit 1
fi

# Check 2: Verify Alpine.js CDN script inclusion
if grep -q "alpinejs" "$INDEX_FILE"; then
    echo "[OK] Alpine.js CDN script verified."
else
    echo "[ERROR] Missing Alpine.js script tag in $INDEX_FILE!"
    exit 1
fi

# Check 3: Ensure zero star/bookmark legacy code references
if grep -i -q "bookmark" "$INDEX_FILE"; then
    echo "[ERROR] Found legacy bookmark reference in $INDEX_FILE!"
    exit 1
else
    echo "[OK] Zero legacy bookmark references found."
fi

# Check 4: Verify single-root templates inside x-for loops (Alpine V3 compliance)
if grep -q "x-for" "$INDEX_FILE"; then
    XFOR_COUNT=$(grep -c "x-for" "$INDEX_FILE" || true)
    TEMPLATE_COUNT=$(grep -c "<template" "$INDEX_FILE" || true)
    if [ "$TEMPLATE_COUNT" -lt "$XFOR_COUNT" ]; then
        echo "[ERROR] Found $XFOR_COUNT x-for directives but only $TEMPLATE_COUNT <template> tags in $INDEX_FILE!"
        exit 1
    fi
    echo "[OK] Verified $XFOR_COUNT x-for directives match $TEMPLATE_COUNT Alpine V3 <template> tags."
fi

echo "========================================="
echo "  ALL FRONTEND CHECKS PASSED SUCCESSFULLY  "
echo "========================================="
