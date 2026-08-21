# Architecture & Data Flow Reference

## Core Infrastructure

1. **Oracle Cloud VPS (Data & Backend Node)** 
   - A single Rust API binary executes through `/usr/local/bin/amrita-server` mapped locally to `localhost:80`.
   - Security constraints mandate execution privileges via `systemd` bypassing user-space strictures.
   - Ingress is managed purely via a background **Cloudflare Tunnel (`cloudflared`)** operating securely over outbound TLS tunnels without requiring permissive ingress internet gateways.

2. **Cloudflare Pages (Frontend & Edge Proxy)**
   - Alpine.js static layout hosted across `exampapersamrita.pages.dev`.
   - To bypass SPA path collisions and browser cache issues overriding standard `_redirects` routing, routing is abstracted cleanly via **Cloudflare Pages Functions**. The `functions/api/[[path]].js` injects secure Cross-Origin validation bridging static user sessions to the dynamically resolving `cloudflared` endpoints.

3. **Storage & Blob Sync**
   - **Database Index:** Sub-millisecond SQLite FTS5 index spanning arrays metadata mapped across strictly 19,600 verified papers. 
   - **OCI Blob Store:** Unstructured binaries mapped to Oracle Free Tier object storage utilizing unified namespaces serving proxy-CDN delivery architectures.
