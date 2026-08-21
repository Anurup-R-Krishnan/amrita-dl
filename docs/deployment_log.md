# Live Deployment Architecture Log

**1. The Architecture Mismatch**
- *Roadblock:* Deploying optimized x86_64 pre-compiled binary via `scp` resulted in systemd immediately failing `(code=exited, status=203/EXEC)` on Ampere A1 (aarch64) structures.
- *Fix:* Abandoned cross-compilation linking for strict local compilation executed natively on the VM target.
-> *Verify: Binary execution bridges exact `aarch64` mapping matching local OS definitions seamlessly.*

**2. Dynamic Remediation: Source Bundling**
- *Roadblock:* Direct `git pull` from Oracle instances fetched unstructured elements bypassing local lockfiles.
- *Fix:* Aggressively compressed explicit dependencies (`src`, `web`, `Cargo.toml`, `Cargo.lock`, `scripts`) avoiding raw repository pulling dynamically into `source.tar.gz`.
-> *Verify: `tar -tvf source.tar.gz` registers exact file dependency boundaries bypassing extraneous `.git` logic.*

**3. In-Flight Data Database Transfer**
- *Roadblock:* Rebuilding `index.db` organically via Python generated SQLite deadlock conditions halting execution flow across network connections natively.
- *Fix:* Enacted direct SCP raw database binary syncing over SSH enforcing locked state integrity matching exactly over local resolutions reliably.
-> *Verify: `sqlite3 index.db "PRAGMA integrity_check;"` yields clean mapping over Oracle target without missing tables natively.*

**4. DNF/Yum Dependency Installation**
- *Roadblock:* Local Cargo `rusqlite` dependencies aborted returning `gcc not found`. Oracle base images ship devoid of build-tools natively.
- *Fix:* Bootstrapped the official Rust toolchain via `rustup` injecting exact GCC C-toolchain dependencies (`gcc clang llvm-devel glibc-devel`) matching the VM dynamically.
-> *Verify: `gcc --version` returns compiler integration flawlessly passing `cargo build` matrices actively matching Linux kernels directly.*

**5. Systemd Permissive Runtimes**
- *Roadblock:* Running binaries compiled under user accounts generated Systemd SELinux `Permission Denied` execution drops.
- *Fix:* Mandated `mv server /usr/local/bin/amrita-server` establishing secure Root-bound permission capabilities retaining mapping structures across boot cycles.
-> *Verify: `systemctl is-active amrita-server` reads explicitly active avoiding previous permission panics natively.*