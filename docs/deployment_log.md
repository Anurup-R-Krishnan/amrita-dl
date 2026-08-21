#  Live Deployment Log & Critical Architecture Fixes

This document serves to extensively accurately record the exact deployment steps, specifically detailing the unexpected architecture mismatches and how they were dynamically resolved for the Oracle Linux target on August 20.

## 1. The Architecture Mismatch
During Phase 1 of the scheduled deployment, the optimized Rust binary was compiled locally. The compilation assumed an `aarch64` (Ampere A1) target based on the initial provisioning guidelines.
- **The Reality:** Oracle Cloud successfully provisioned a `VM.Standard.E2.1.Micro` instance. This uses a standard AMD/Intel `x86_64` CPU.
- **The Symptom:** When transferring the pre-compiled binary via `scp`, the `systemd` daemon immediately failed with an error `(code=exited, status=203/EXEC)`. 

## 2. Dynamic Remediation: The Native Compile Strategy
Cross-compiling C-dependencies and `glibc` libraries across heavily divergent Linux distributions (Arch Linux locally vs. Oracle Linux 9 remotely) can cause obscure linking failures. 

To create a perfectly stable, 100% native environment, the deployment strategy was pivoted entirely on the fly:
1. **Source Bundling:** The local source code (`src`, `Cargo.toml`, `Cargo.lock`, `scripts`) was dynamically aggressively compressed into a `source.tar.gz` archive.
2. **In-Flight Transfer:** The compressed source code and the newly fixed 19,600-row `index.db` were silently pushed to the `opc@140.245.237.213` server.
3. **VM Native Bootstrap:** An automated SSH script ran on the Oracle VM to instantly transform it into a Rust build server:
   - Installed `gcc` and `sqlite-devel` OS packages via `dnf`.
   - Bootstrapped the official Rust toolchain via `rustup`.
   - Unpacked the source and ran `cargo build --release --bin server`.
4. **Daemon Launch:** Once compiled inside the VM natively against Oracle Linux 9 libraries, the process became absolutely durable, and `systemd` was restarted to bind to port 80.

## 3. Firewall Orchestration
Oracle Linux utilizes `firewalld` internally. The automated bash routine dynamically added port `80/tcp` to the `public` zone permanently, enabling web traffic to bypass OS-level restrictions immediately.

## 4. Frontend Reverse Proxy Rollout
Because the Oracle API server was bound to 140.245.237.213, local CORS constraints had to be bypassed entirely to serve the frontend via Cloudflare Pages.
1. The `web/_redirects` file was securely overridden:
   `/api/* http://140.245.237.213/api/:splat 200`
2. The entire `web/` folder was directly uploaded utilizing `wrangler pages deploy --project-name exampapersamrita` via Direct Upload (bypassing GitHub temporarily for speed).

*Result:* Browsing the Cloudflare edge automatically hits Oracle servers invisibly.

## 5. ARM64 Capacity Starvation (Re-Pivot)
After attempting to fix the `x86_64` to `AARCH64` translation, the `ap-hyderabad-1` data center refused provisioning of the `VM.Standard.A1.Flex` shape with `Out of host capacity`. This is severely common for Oracle's free ARM tier.
To guarantee deployment today, the setup pivoted *back* to the `VM.Standard.E2.1.Micro`. 

## 6. OOM Mitigations (Swap and Throttling)
To prevent the micro instance from dropping SSH sessions or Kernel Panicking entirely when compiling `axum`/`tokio` locally:
1. Natively injected a 4 GB Swap memory fallback via `mkswap`.
2. Halted aggressive CPU multi-threading by appending `-j 1` to strictly force single-core `cargo` pipelines.
3. Decoupled process life-cycling by encapsulating it cleanly internally with `nohup ... &`.
