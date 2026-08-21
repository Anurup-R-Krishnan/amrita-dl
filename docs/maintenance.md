# Operations & Maintenance Guide

## 1. Local Development

Legacy bash hooks (`run_local.sh`) are entirely deprecated.

**Run server natively**
```bash
cargo run --release --bin server
```
*Verify: `curl -I http://localhost:3000/api/search?q=test` returns HTTP 200.*

## 2. Syncing Data and Blobs

Previous Python scripts (`perfect_sync.py`, `upload_pdfs.py`) produced database deadlock exceptions and unescaped shell paths. Use the rewritten native Rust binaries matching local environments securely.

**Align Database Index**
```bash
cargo run --release --bin db_sync
```
*Verify: `sqlite3 index.db "SELECT count(*) FROM papers;"` yields 19,600+ validation count.*

**Object Storage Push**
```bash
cargo run --release --bin upload
```
*Verify: Rclone payload output matches DB index parity with zero error warnings.*