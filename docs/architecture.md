# ️ Architecture & Data Flow Reference

This document serves as the absolute source of truth for the Amrita Exam Papers search engine architecture, having been verified against the live Rust codebase and cloud configuration.

## System Topology

The system operates across a strictly zero-cost production stack, split into three distinct layers:

1. **Frontend / Edge Layer (Cloudflare Pages)**
   - **Hosting:** Static HTML/JS is served instantly from Cloudflare edge caches.
   - **CORS Bypass:** The `web/_redirects` file seamlessly intercepts `/api/*` requests and reverse-proxies them to the backend (`https://api.amritapapers.dev`). The browser views the API as "same-origin", entirely bypassing complex CORS preflight issues in production.
   - **Domain:** Hosted at `exampapersamrita.pages.dev` (with standard Cloudflare TLS).

2. **Compute / Backend Layer (OCI ARM VM + SQLite)**
   - **Infrastructure:** Oracle Cloud Always Free Ampere A1 (ARM64) running an optimized Rust binary `amrita-server`.
   - **Source of Truth:** A highly optimized `index.db` (SQLite FTS5) holds metadata for exactly 19,600 unique exam papers.
   - **Redirection Engine:** When a user requests a PDF (`/api/pdf?id=...`), the server checks the database, finds the `relative_path`, safely encodes the URL segments, and instantly returns an HTTP `302 Found` redirect.

3. **Storage Layer (OCI Object Storage)**
   - **Bucket:** `oracle-amrita-bucket` deployed in `ap-hyderabad-1` (Visibility: Public).
   - **Direct Delivery:** Because of the backend's `302` redirect, the user's browser downloads the PDF *directly* from Oracle's massive bandwidth pool. The tiny OCI Compute VM never touches the PDF bytes.

## Critical Design Decisions

### 1. Batch Download Limitation (501 NOT IMPLEMENTED)
If the server is started with the `STORAGE_PUBLIC_URL` environment variable (Production Mode), the `/api/download-batch` endpoint is intentionally disabled (returning `501`). Because the backend VM does not keep the 9.2 GiB of PDFs on its local disk, it cannot dynamically stitch together `.zip` files for users. 

### 2. The Great Path Duplication
While there are ~19,600 *unique* physical PDFs (representing ~5.3 GiB), the indexing script expanded these into ~29,678 paths (9.15 GiB) before uploading to Cloud Storage. This intentional denormalization occurs because "Shared Courses" (e.g., a common Math paper) are duplicated into the departmental folders for Biotech, CompSci, etc. 

### 3. The Death of `.meta` files
During local processing, the indexer generates thousands of `.meta` sidecar files (JSON files mapping PDFs to DB records). While these exist on local external drives, they are **excluded** from the OCI bucket sync (`--exclude "*.meta"`). The `index.db` file is the sole, absolute source of truth required for the server to function.
