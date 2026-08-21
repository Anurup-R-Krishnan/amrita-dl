# Amrita Exam Papers Search Engine

A high-performance, fully searchable archive of 19,600+ Amrita Vishwa Vidyapeetham examination question papers spanning B.Tech, M.Tech, MCA, M.Sc, and PhD programs.

## System Architecture

The search engine is built around a zero-cost, enterprise-grade cloud architecture:

1. **Frontend (Cloudflare Pages)**
   - Alpine.js structured SPA served globally from the Cloudflare Edge network.
   - Domain: `exampapersamrita.pages.dev`
   - Cross-Origin proxy handled internally via Cloudflare Pages Functions (`functions/api/[[path]].js`).

2. **Backend (Oracle Cloud Linux VM)**
   - Custom asynchronous high-performance Rust web server traversing a local SQLite FTS5 database.
   - The OS firewall restricts public inbound web traffic ensuring direct IP hiding.
   - **Cloudflare Tunnel (`cloudflared`)** establishes an outbound TLS tunnel directly to the Cloudflare Edge masking the Oracle Cloud server IP securely.

3. **Blob Storage (Oracle Object Storage)**
   - Stores over 9 GB of raw PDF binary objects.
   - The backend maps Search Index `relative_path` mappings dynamically to Cloudflare proxied CDN routes pointing safely back to securely configured Oracle buckets avoiding public S3 listing exposures.

## Development Pipeline

1. **Building the Rust Backend Locally**
   ```bash
   # Compile natively
   cargo build --release --bin server

   # Run locally (Listens on port 3000 by default)
   cargo run --release --bin server
   ```

2. **Syncing New Exams & Data**
   Instead of unreliable Python/Bash shell interpolations, dataset modifications happen natively in compiled robust environments:
   ```bash
   # Synchronize database constraints
   cargo run --release --bin db_sync

   # Upload PDFs via Rclone interfacing
   cargo run --release --bin upload
   ```

## Production Automation

The system employs continuous deployments through **GitHub Actions**. Pushing directly to the `main` branch immediately builds tests against the Rust repository utilizing `clippy`, and seamlessly forces JWT-secured direct static manifest distribution of the `web/` tree straight into the Cloudflare Pages edge deployments.

For granular infrastructure logs mapping exactly how memory limits and port escalation issues were navigated on Oracle Free Tier instances, please reference `/docs/production_remediation_postmortem.md`.