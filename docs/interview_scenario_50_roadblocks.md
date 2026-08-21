# Technical Interview Scenario: Resolving 50 Complex Production Roadblocks

**Candidate Profile:** Systems Architect / DevOps Engineer
**Project Context:** Migrating the Amrita Exam Papers search engine (Rust, SQLite, OCI Linux, Cloudflare Pages/Tunnels, CI/CD) from localized/Render architectures to absolute zero-cost native cloud primitives.

Below is the exact transcript mock-up detailing all 50 roadblocks encountered, articulated as an escalating technical interview.

---

## Part 1: Compute & Compilation Constraints

**Interviewer: We provisioned an Oracle Free Tier VM (1GB RAM). How did you prevent Rust compilation from crashing the server?**

**Candidate:**
1. **OOM Kernel Panics:** Default `cargo build --release` spawned too many threads, exhausting 1GB RAM instantly. Throttled concurrency strictly via `cargo build -j 1`.
2. **Link Time Optimization Memory Spikes:** The `[profile.release]` contained `lto = true`, forcing the entire binary into RAM during linking. Disabled (`lto = false`).
3. **Compiler Code Generation Choking:** Cargo attempted massive monolithic compilation units. Injected `codegen-units = 16` into `Cargo.toml`.
4. **Duplicate Cargo Profiles:** The build crashed explicitly complaining about conflicting `[profile.release]` blocks. Merged the duplicate tables.
5. **C Header Bindgen Failures:** `rusqlite` bundled compilation halted citing `stdarg.h not found`. Installed missing C-toolchain dependencies: `gcc clang llvm-devel glibc-devel`.
6. **Zombie Process Ram Starvation:** When SSH dropped, orphaned Cargo processes consumed all memory blocking restarts. Injected strict `pkill -f 'cargo build'` hooks before any compilation retry.

## Part 2: Deployment Orchestration & Execution

**Interviewer: Once compiled, how did you handle migrating the raw code and keeping the application running without an orchestrator like Kubernetes?**

**Candidate:**
7. **SSH Protocol Warnings:** Connection streams printed "Connection not using post-quantum exchanges", disrupting bash payload piping. Aggregated SSH logic filtering errors using explicit grep inversions.
8. **Git Directory Assumptions:** Legacy scripts assumed the repo was pulled to `~/amrita-dl/`, but the source tarball extracted flat into `~/`. Re-mapped all paths relative to the user root.
9. **Missing Tarball Assets:** The frontend `web/index.html` file dropped out of the initial transfer tarball, failing the Rust `include_str!` macro. Re-bundled `source.tar.gz` explicitly mapping the `web/` node.
10. **Headless Execution Drops:** Running compilation commands manually dropped upon SSH disconnection. Implemented `nohup cargo build > build.log &` storing the PID dynamically to `build.pid`.
11. **Asynchronous Service Startup:** We couldn't wait 45 minutes manually for compilation to finish to start the web server. Authored detached polling loop testing for `target/release/server` existence every 30 seconds to trigger `systemctl`.
12. **Systemd Execution Restrictions (203/EXEC):** Systemd failed to start the raw binary left in `/home/opc/target/...` throwing `Permission Denied` due to SELinux user-space restrictions. Relocated executable to `/usr/local/bin/amrita-server`.
13. **Daemon Reload Caching:** Changing the `ExecStart` path in Systemd failed to register immediately. Forced `systemctl daemon-reload` before enabling.

## Part 3: Overcoming Data Integrity and Shell Errors

**Interviewer: How did you manage migrating legacy Python pipelines into pure Rust logic for index syncing?**

**Candidate:**
14. **Legacy Perfect Sync DB Locking:** `perfect_sync.py` failed during multi-threaded writes returning "database is locked". Rewrote integration into `db_sync.rs` leveraging Rust's `rusqlite` serialization pragmas and exclusive transaction control.
15. **Rclone Shell Injection Vulnerabilities:** `upload_pdfs.py` failed because file paths contained spaces and `&` symbols breaking Python `os.system`. Rewrote to `upload.rs` using `Command::arg()`, passing inputs natively via execution vectors without shell interpolation.
16. **Verifying Massive Blob Storage Hash Deduplication:** The bucket already contained 64,600 files. Utilized `rclone size` across the Oracle target validating exact byte totals matching the database index parity.
17. **SQLite Index Portability Risks:** Migrating a 13MB `index.db` via standard cloud volumes risked corruption. Enacted direct SCP transfers via the validated local machine bypass.
18. **Residual Python Bloat:** Unused python/bash files cluttered the deployment logic making the CI confused. Invoked `git rm` scrubbing all non-Rust syncing solutions universally.

## Part 4: Navigating Network & Origin Restrictions

**Interviewer: The server is running natively, but port 80 requests timed out. Walk me through debugging the network bridging.**

**Candidate:**
19. **Firewalld IP Drops:** Internal `curl` on the VM resolved JSON, but eternal IPs dropped. Checked `sudo firewall-cmd --list-ports` indicating Port 80 was sealed at the Oracle subnet layer.
20. **Restricted Edge Origin Rules:** Standard Cloudflare Pages `_redirects` dropped proxy routing to `http://68.233.111.2`. Discovered Pages 200 proxies require standard HTTPS compliant target endpoints.
21. **Cloudflare RPM Distribution 404s:** Standard `dnf` and `rpm` package requests for `cloudflared` failed retrieving obsolete repository links. Pulled native AMD64 compiled binaries natively over `wget/curl`.
22. **Privileged Escalation of Daemons:** The `cloudflared` executable was barred from opening port configurations internally. Pushed binary to `/usr/local/bin` and invoked `chmod +x` enforcing root-level capability mappings.
23. **Temporary Tunnel Output Parsing:** Ephemeral `.trycloudflare.com` URLs generated asynchronously in logs. Constructed exact `grep -o 'https://[a-z0-9-]*\.trycloudflare\.com'` syntax fetching dynamic routing URLs blindly.
24. **Cloudflare Worker 1003 Banning:** A testing Cloudflare worker failed explicitly with error 1003 restricting cross-zone unencrypted origins. This necessitated adopting the Cloudflare Tunnel bridge exclusively.
25. **Named Tunnel Creation Collisions:** API requested the establishment of a static tunnel `amrita-api` which crashed citing duplicate naming entries. Retrospectively extracted the existing `a0a5a18a-fc04...` UUID instead.
26. **Named Tunnel Unauthenticated Ingress:** Starting the UUID tunnel required massive 168-character tokens unreachable by frontend UIs. Acquired the respective JWT by tapping `api.cloudflare.com/client/v4/accounts/{id}/cfd_tunnel/{id}/token`.
27. **IPv6 Disallowed Routing Errors:** The correctly connected named tunnel threw HTML error pages citing IPv6 mapping deficiencies. Solved by updating the explicit API `ingress` routing rules mapping traffic forcibly into local `http://localhost:80`.
28. **Systemd Cloudflared Persistence:** Ephemeral TryCloudflare endpoints self-destruct post-reboot destroying proxy targets. Wrote `cloudflared-tunnel.service` guaranteeing eternal startup of the HTTPS network bridge.

## Part 5: Resolving Frontend & CI/CD Cloudflare Integration

**Interviewer: Explain resolving the Single Page Architecture (SPA) collision points against Cloudflare proxy systems.**

**Candidate:**
29. **API Parsing HTML Errors:** The frontend UI violently threw JSON parse errors. `curl`ing the origin endpoint exposed it returning `index.html` source codes instead of DB queries.
30. **Pages Catch-All Override:** Cloudflare Pages treats active logic branches as SPA fallbacks natively superseding `_redirects`. Scrapped static redirectors substituting programmatic interceptors.
31. **Developing Full Cloudflare Functions:** Bypassed `_redirects` by coding `[[path]].js` targeting the tunnel destination leveraging programmatic Cross-Origin resolution natively.
32. **Functions Directory Obfuscation:** The `/functions/` node placed accidentally under `/web/functions/` was entirely ignored by the Wrangler builder matrix. Relocated to the parent `/functions/` root directory enabling edge integration.
33. **Pre-flight CORS Restraints (OPTIONS):** The `fetch` calls intercepted by Functions threw invalid CORS requests missing Option mappings. Interrogated the Function code and securely returned HTTP 204 intercepts validating `Access-Control-Allow-Origin: *`.
34. **Fallback API Parsing Toggles:** Frontend code utilized empty string fallbacks natively processing local routes via `getApiBase()`. Ensured zero-length URL injection appended flawlessly without duplicating forward slashes.

## Part 6: Overriding Obsolete Wrangler Configurations

**Interviewer: The GitHub Action pipeline crashed continuously. How did you restore automation deployments?**

**Candidate:**
35. **Obsolete Bash Invocation Hooks:** The GitHub Action attempted executing a locally deleted `check_frontend.sh` file halting the build pipeline. Audited `.github/workflows/ci-cd.yml` extracting ghost bash invocations ensuring pure Rust integration runs.
36. **Render Platform Migration Ghosts:** Native workflows continually attempted Render remote deploy pushes via empty Secret Webhook URLs. Discarded the Render fallback topology totally relying on Pages Edge infrastructures.
37. **Cloudflare Account ID Ambiguity:** Multiple Cloudflare pages domains existed (`amritapapers-1x5` vs `exampapersamrita`). Evaluated REST API endpoints parsing exact UUIDs aligning `exampapersamrita` across Wrangler definitions.
38. **Wrangler Auth Cache Poisoning (Code 10000):** Wrangler generated fatal local Authentication failures on valid domains executing actions. Diagnosed token expiration matrices skipping manual local OAuth commands.
39. **API Injection vs OAuth Refreshing:** Automated CI runners cannot authorize OAuth prompts. Engineered an explicitly masked bypass `CLOUDFLARE_API_TOKEN=$CF_TOKEN wrangler deploy`.
40. **Dirty Staging Environment Blocking:** Untracked repository files (`.wrangler/` generated locally) aborted CI pushes automatically. Instructed execution overriding `--commit-dirty=true` natively inside actions deployments enforcing continuous merges.
41. **Uploading Manifest Anomalies:** Attempting manual Pages pushing via raw Cloudflare API generated `{code: 8000096, message: manifest missing}`. Validated binary hashing mechanics were obsolete, preferring raw Wrangler configurations natively.

## Part 7: Repository Hygiene & Automation Security

**Interviewer: The application works completely natively now under zero costs. But how did you repair the engineering environment hygiene itself?**

**Candidate:**
42. **Massive Git Repository Leakages:** Evaluated `git status` extracting catastrophic security risks: Private RSA deployment keys (`ssh-key-2026-08-20.key`), massive Database Wal files, and backend server logs were entirely untracked and facing potential global commits.
43. **Gitignore Rectification:** Built aggressive `.gitignore` constraints forcibly rejecting `*.key`, `*.pem`, `/target/`, `.db-shm/wal` protecting structural compliance tracking formats forever from injection failures.
44. **Erasing Generative AI Slop Documentation:** The main `.README` files explicitly output "As an AI language model I noticed...". Enacted brutalist scrubs across configurations erasing hallucination artifacts directly formatting technical facts only.
45. **Strict Unicode / ASCII Enforcement:** CI documentation contained emoji layouts (☁ / ️) restricting pure deployment parsifiers. Ran systemized regex extractions flattening architectures to exact ASCII structural standards.
46. **Markdown Header Nullification Gaps:** Removing Emojis corrupted the markdown hierarchical boundaries resulting in double whitespacing (e.g. `#  Headers`). Restored precise regex sed substitutions recovering syntactically accurate files internally.
47. **Multiple Outdated Documentation Vectors:** The core setup guide pointed to `run_local.sh` and 140.x remote IPs which crashed integrations persistently. Extensively rewrote `deployment_log.md`, `deploy_oci.md` synchronizing to exactly the present configuration state natively without bloat matrices.
48. **Validating Endpoint Caching Failovers:** Real endpoints resolved flawlessly locally but browsers loaded cached HTML errors (Apollo JSON crashes). Enforced rigorous `curl -I` validation ensuring JSON `content-type` returns isolated browser caches definitively.
49. **Enforcing Brutalist Operational Architectures:** Transitioned subjective deployment decisions applying Karpathy Operational Frameworks. Stripped away "flexibility and speculative wrappers" strictly verifying commands independently over guessing variables autonomously.
50. **Securing Autonomous AI Failure Loops:** Rewrote the entire Agent Guidelines internal framework guaranteeing explicit closed-loop validations, demanding any modifications be restricted strictly to one-line diff formats restricting unmonitored architecture modifications structurally.