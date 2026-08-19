# Amrita Exam Papers -- Enterprise Pure-Cloud Deployment Guide (OCI + Cloudflare)

This guide details how to host the Amrita Exam Papers Search Engine completely free ($0/month forever) with zero cold starts, persistent SQLite FTS5 search indexing, low-latency edge CDN delivery, and 100% bandwidth offloading.

---

## Production Stack: OCI Compute + OCI Object Storage + Cloudflare Pages ($0.00 / mo)

| Component | Service | Cost | Function |
| :--- | :--- | :--- | :--- |
| **Frontend SPA** | Cloudflare Pages | **$0.00 / mo** | Global edge network hosting for `web/index.html` (`amritapapers.pages.dev`) |
| **Backend API Engine** | OCI Always Free VM | **$0.00 / mo** | Native Rust Axum backend + SQLite FTS5 search on persistent disk (0 cold starts) |
| **PDF Object Storage** | OCI Object Storage | **$0.00 / mo** | 200 GB Always Free object storage for 29,600+ PDF question papers |
| **Bandwidth (Egress)** | OCI + Cloudflare | **$0.00 / mo** | 10 TB/month free egress from OCI + Cloudflare Edge Caching |

### Architecture & Zero-Cost Egress Flow

```
[ User Browser ]
       │
       ├────────────────────────────────────────┐
       │ 1. Static SPA (amritapapers.pages.dev)  │ 2. Search API (/api/* via Cloudflare proxy)
       v                                        v
[ Cloudflare Pages Edge ]              [ OCI Always Free VM ]
(275+ Global Data Centers)             (Rust Axum Engine + index.db)
                                                │
                                                │ 3. HTTP 302 Redirect
                                                v
                                     [ OCI Object Storage ]
                                     (oracle-amrita-bucket in ap-hyderabad-1)
```

---

## Step 1: Upload Question Paper Dataset to Oracle Cloud (OCI)

Sync the 29,678 PDF question papers to your OCI bucket (`oracle-amrita-papers:oracle-amrita-bucket`) via `rclone`:

```bash
# Test sync with dry run (excludes .meta sidecar files)
./scripts/sync_to_oracle.sh --dry-run

# Perform live dataset upload
./scripts/sync_to_oracle.sh
```

---

## Step 2: Build Lightweight Backend Deployment Package

On your build machine, generate the lightweight deployment bundle (contains `server` release binary + `index.db` + systemd service unit):

```bash
SKIP_PDF_COPY=1 ./scripts/prepare_cloud_deploy.sh
```

The prepped bundle is created at `./dist`.

---

## Step 3: Deploy Backend Axum Daemon on OCI VM (Systemd)

1. SSH into your OCI Always Free Ubuntu/Debian Instance:
   ```bash
   ssh ubuntu@<YOUR_OCI_VM_IP>
   ```

2. Copy `./dist` contents to `~/amrita-app`:
   ```bash
   scp -r ./dist/* ubuntu@<YOUR_OCI_VM_IP>:~/amrita-app/
   ```

3. Install and enable the `amrita-server.service` systemd daemon:
   ```bash
   sudo cp ~/amrita-app/amrita-server.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable --now amrita-server
   ```

4. Check daemon status:
   ```bash
   sudo systemctl status amrita-server
   curl http://localhost:8080/api/health
   ```

---

## Step 4: Deploy Static SPA Frontend to Cloudflare Pages

Deploy the static web assets to Cloudflare Pages (`amritapapers.pages.dev`):

```bash
npx wrangler pages deploy web --project-name amritapapers
```

Cloudflare Pages uses `web/_redirects` to proxy `/api/*` requests directly to your backend API domain, eliminating CORS overhead.
