# Architecture & Data Flow Reference

## The Motive and The Struggle
When we set out to build an architecture capable of instantly searching and filtering through an archive of 19,600+ Amrita Vishwa Vidyapeetham examination papers, the mandate was clear: absolute maximum efficiency, absolute zero cost. 

We planned to deploy on Oracle's free-tier Ampere A1 (24GB RAM, 4 ARM Cores), feeding a lightning-fast Rust Axum backend with an embedded SQLite FTS5 (Full-Text Search) engine. But engineering reality hit us like a freight train. The OCI `ap-hyderabad` region was completely tapped out of Ampere capacity. We couldn't get a 24GB node. Out of pure desperation, we fell back to the 1GB RAM, x86_64 legacy `VM.Standard.E2.1.Micro` tier. 

This single limitation cascaded into a nightmare of technical debt. We tried cross-compiling on CachyOS, only to hit fatal `glibc` linking panics that rendered our binaries completely dead on arrival yielding `203/EXEC`. We pivoted to native compilation on the Oracle VM, only for `cargo build` to trigger instant kernel panics as `rustc` utterly starved the 1GB memory limit. Our infrastructure had to be aggressively hacked: 4GB artificial swap spaces, extreme cargo thread strangulation (`-j 1`), and routing workarounds because Cloudflare Pages native routes overrode our `_redirects`.

The architecture below is not just a diagram; it is an artifact of survival. It represents the exact configuration required to run an enterprise-grade academic search engine on hardware weaker than a modern smartphone.

## Core Infrastructure

1. **Oracle Cloud VPS (Data & Backend Node)** 
   - A perfectly compiled single Rust API binary executes through `/usr/local/bin/amrita-server` mapped locally to `localhost:80`.
   - Security constraints mandate execution privileges via `systemd` bypassing user-space strictures completely avoiding permissions panics.
   - Ingress is managed purely via a background **Cloudflare Tunnel (`cloudflared`)** daemon. By operating securely over outbound TLS tunnels, we completely avoid permissive internet gateways or manual iptable firewall nightmare configurations.
   -> *Verify: `cloudflared tunnel info` logs active TLS mapping dynamically reflecting secure state paths.*

2. **Cloudflare Pages (Frontend & Edge Proxy)**
   - Alpine.js static layout hosted across `exampapersamrita.pages.dev`.
   - The major pitfall with Cloudflare Pages was encountering active SPA route collisions; standard `_redirects` routing failed silently because DOM overrides intercepted the paths.
   - We aggressively rewrote routing into explicit **Cloudflare Pages Functions**. The `functions/api/[[path]].js` injects secure Cross-Origin validation bridging static user sessions to the dynamically resolving `cloudflared` endpoints safely.
   -> *Verify: Execution resolves 200 HTTP directly bypassing XML intercepts executing native JSON payloads accurately.*

3. **Storage & Blob Sync**
   - **Database Index:** Sub-millisecond SQLite FTS5 index spanning array metadata mapped rigidly across exactly 19,600 verified papers. 
   - **OCI Blob Store:** Unstructured binaries (9+ GB of raw pdfs) are mapped natively to Oracle Free Tier object storage utilizing unlisted namespaces, delivering through proxy-CDN streaming without exposing bucket internals.
   -> *Verify: SQLite matches payload counts universally validating FTS indexing accurately.*