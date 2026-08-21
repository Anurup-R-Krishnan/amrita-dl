# Live Deployment Log & Critical Architecture Fixes

This document records deployment steps, specifically detailing unexpected architecture mismatches dynamically resolved for the Oracle Linux target on August 20-21.

## 1. The Architecture Mismatch
During Phase 1 of the scheduled deployment, the optimized Rust binary was compiled locally assuming an `aarch64` (Ampere A1) profile.
- **The Reality:** Oracle Cloud successfully provisioned a `VM.Standard.E2.1.Micro` instance standard AMD/Intel `x86_64` CPU.
- **The Symptom:** Transferring the pre-compiled binary via `scp` resulted in systemd immediately failing `(code=exited, status=203/EXEC)`. 

## 2. Dynamic Remediation: The Native Compile Strategy
Cross-compiling C-dependencies and `glibc` libraries across heavily divergent Linux distributions triggers obscurity linking panics. 

To create a perfectly stable native environment, the core logic was pivoted:
1. **Source Bundling:** The local source code (`src`, `web`, `Cargo.toml`, `Cargo.lock`, `scripts`) was dynamically aggressively compressed into a `source.tar.gz` archive.
2. **In-Flight Transfer:** The compressed source code and the newly fixed 19,600-row `index.db` were silently pushed directly to the VM via direct Secure Copy Protocol avoiding Rclone string manglings.
3. **VM Native Bootstrap:** SSH execution ran on the Oracle VM natively building dependencies:
   - Installed `gcc`, `clang`, `llvm-devel`, and `glibc-devel` OS packages via `dnf` to supply headers for `rusqlite` execution formats.
   - Bootstrapped the official Rust toolchain via `rustup`.
   - Executed limited compilation mapping `codegen-units=16` avoiding aggressive kernel RAM kills on 1GB instances.
4. **Daemon Security Paths:** Executables inside `/home/opc` fail execution layers within SystemD constraints. Binaries were aggressively moved to `/usr/local/bin/amrita-server` establishing persistent local Port 80 binds safely.