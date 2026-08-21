# Amrita Exam Papers Search Engine

Searchable archive of 19,600+ Amrita Vishwa Vidyapeetham examination papers.

## Architecture

1. **Frontend:** Cloudflare Pages (`exampapersamrita.pages.dev`). Proxies `/api/*` to OCI via Pages Functions (`functions/api/[[path]].js`).
2. **Backend:** Oracle Cloud Linux VM (Rust Axum + SQLite FTS5). Port 80 exposed exclusively via Cloudflare Tunnel (`cloudflared`).
3. **Storage:** Oracle Object Storage (9+ GB PDFs). Accessed via relative CDN routing mappings; public bucket listing disabled.

## Development

**1. Build backend**
```bash
cargo build --release --bin server
```
*Verify: `ls -lh target/release/server` confirms binary exists.*

**2. Run backend**
```bash
cargo run --release --bin server
```
*Verify: `curl -s http://localhost:3000/api/facets` returns JSON payload.*

**3. Sync schema/database**
```bash
cargo run --release --bin db_sync
```
*Verify: `sqlite3 index.db "PRAGMA integrity_check;"` returns `ok`.*

**4. Upload raw objects**
```bash
cargo run --release --bin upload
```
*Verify: `rclone size oracle-amrita-papers:oracle-amrita-bucket` count matches index row count.*

## Continuous Integration

Pushes to `main` trigger `.github/workflows/ci-cd.yml` restricting failures upstream. 
*Verify: GitHub Actions pipeline resolves without error; `exampapersamrita.pages.dev` reflects raw changes within ~10 seconds.*