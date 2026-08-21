# Compute & Compilation Constraints

**1. OOM Kernel Panics**

*The Pitfall (Deep Context):*
The Oracle `VM.Standard.E2.1.Micro` ships with exactly 1GB of physical RAM and 1 vCPU. We had originally targeted the Ampere A1 Flex shape with 24GB of RAM and 4 ARM cores, a machine where Rust compilation is routine. That shape was completely unavailable in `ap-hyderabad` for weeks due to Oracle Free Tier demand exhaustion.

On the 1GB fallback machine, we ran a completely default `cargo build --release`. Cargo detects system threads and spawns parallel `rustc` worker processes, one per available logical CPU thread. Even with 1 vCPU, Cargo's job scheduler spins up multiple overlapping `rustc` invocations during the dependency graph resolution phase. Each `rustc` invocation for a moderately complex crate allocates between 80-200MB of RAM for AST construction, type checking, and MIR lowering.

Within 14 seconds of starting, `htop` showed memory at 990MB. Then the kernel OOM killer woke up. It does not pause processes. It does not swap gracefully. It sends `SIGKILL -9` to the highest-memory process immediately. The target was `rustc`. But because the entire OS networking stack was simultaneously memory pressured, the SSH session froze before `rustc` even received the signal. The Oracle OCI dashboard eventually reported the VM status as "Terminated." The kernel had crashed and rebooted the entire node to recover. We learned that compiling Rust on a 1GB machine without aggressive throttling was physical suicide for the server.

*How we faced it:*
We added `-j 1` to every cargo invocation. This flag sets the number of parallel job slots to exactly 1, forcing cargo to compile one crate at a time, sequentially. Memory usage at peak compilation dropped from the fatal 990MB to a stable ceiling of around 650MB, leaving enough headroom for the OS, sshd, and Systemd to remain functional. Compile time increased from a theoretical 8 minutes to over 45 minutes, but the binary actually finished building. We also created a 4GB swap file first (see Scenario 6), which provided a secondary safety net for the occasional spike above 650MB during macro expansion in crates like `tokio` and `serde_derive`.

-> *Verify: `cargo build -j 1` completes without the SSH session dropping and `free -h` shows swap usage rather than OOM kills.*


**2. Link Time Optimization Memory Spikes**

*The Pitfall (Deep Context):*
Even with `-j 1` stabilizing the per-crate compilation phase, we hit a wall at the very final step: linking. Our `Cargo.toml` was configured with `lto = "fat"`. Fat LTO instructs the compiler to defer all optimization work to the link stage. Instead of optimizing each crate in isolation, LLVM waits until all crates are compiled into LLVM Intermediate Representation (IR) bitcode, then loads the entire combined IR of every single dependency into RAM simultaneously to perform cross-crate inlining and dead code elimination.

At compile step `[152/152]`, the linker started loading IR. RAM climbed from 400MB to 680MB to 1.1GB. At 1.1GB the swap kicked in. At 1.6GB the swap I/O on the Oracle SSD became so saturated that the linker process stalled completely, then the kernel OOM killer fired again and killed the linker. The binary never completed. We wasted 45 minutes of compile time every single time this happened.

*How we faced it:*
We set `lto = false` in `[profile.release]`. Without LTO, each crate is optimized independently during its own compilation step, and the linker only needs to combine pre-optimized object files rather than re-optimize the entire combined IR. Peak memory during linking dropped to approximately 850MB, comfortably under the 1GB physical ceiling even without heavy swap involvement. The final binary is marginally larger and marginally slower at runtime, but it actually exists, which is the prerequisite for serving any requests at all.

-> *Verify: `grep "^lto" Cargo.toml` returns `lto = false` and the build completes without the linker being killed.*


**3. Compiler Code Generation Choking**

*The Pitfall (Deep Context):*
After resolving the LTO crash, we noticed compilation had become absurdly slow even by the already-low bar of `-j 1`. The `regex` crate alone was taking 41 minutes. `serde_derive` was taking 28 minutes. These are crates that take 30 seconds on a normal machine.

The root cause was `codegen-units`. By default, Cargo uses `codegen-units = 16` during debug builds but `codegen-units = 1` during release builds. With `codegen-units = 1`, the entire crate's MIR is handed to LLVM as a single monolithic unit. LLVM attempts to analyze all functions simultaneously to find cross-function inlining and optimization opportunities. On a crate like `regex` with thousands of generated state machine functions, this produces a single LLVM module that the optimizer must hold entirely in cache simultaneously. The Oracle E2.1.Micro has a tiny CPU cache. It was spending 95% of clock time evicting and reloading cache lines rather than doing any actual optimization work.

*How we faced it:*
We set `codegen-units = 16` explicitly in `[profile.release]`. This splits each crate's MIR into 16 smaller independent LLVM modules before handing them to the optimizer. Each module is small enough to fit reasonably well in the available cache. LLVM can optimize each chunk quickly without constant cache pressure. The `regex` crate compilation time dropped from 41 minutes to 3 minutes. Total build time dropped from over 4 hours to 47 minutes. The trade-off is that LLVM cannot perform cross-function inlining between the 16 modules, producing slightly less optimal machine code. For a search engine serving academic PDFs over SQLite FTS5, this is entirely acceptable.

-> *Verify: `grep "codegen-units" Cargo.toml` returns `codegen-units = 16` and the `regex` crate compiles in under 5 minutes.*


**4. Duplicate Cargo Profiles**

*The Pitfall (Deep Context):*
During the back and forth of testing LTO settings and codegen-units values, we were editing `Cargo.toml` repeatedly via `vim` over a laggy SSH connection on a machine that had been rebooting every 20 minutes. Human error under stress: we accidentally appended a second `[profile.release]` block to the bottom of the file while the original remained at the top. TOML does not allow duplicate table headers.

We kicked off the next build attempt and Cargo immediately rejected the manifest:
```
error: manifest contains duplicate key `profile.release` for key `profile` in table `profile`
```
We had just wasted another potential compile slot on a syntax error that could have been caught in under a second.

*How we faced it:*
We merged both blocks manually in `vim`, keeping the correct values (`lto = false`, `codegen-units = 16`, `opt-level = 3`) in a single `[profile.release]` table at the top of the manifest. We also added a `cargo check` step before every subsequent `cargo build` invocation. `cargo check` parses the manifest and type-checks the Rust code without producing any compiled output. It runs in under 5 seconds and catches all syntax and type errors before we commit to a 45-minute build.

-> *Verify: `cargo check` exits with code 0 in under 10 seconds before any `cargo build` invocation.*


**5. C Header Bindgen Failures**

*The Pitfall (Deep Context):*
The pure-Rust crates compiled without incident. Then `cargo build` reached `rusqlite`. This crate provides safe Rust bindings to the SQLite C library. It uses a Rust build script (`build.rs`) that invokes the system C compiler to compile the embedded SQLite C source files. The build script invoked `cc`, which attempted to find `gcc` on the system PATH. Oracle Linux's minimal base image ships with zero C development tools. The exact error:
```
error: failed to run custom build command for `rusqlite vX.Y.Z`
caused by: process didn't exit successfully (exit code: 1)
--- stderr
src/sqlite3.c:25:10: fatal error: stdarg.h: No such file or directory
```
`stdarg.h` is part of the C standard library headers provided by `glibc-devel`. Without it, the SQLite C source cannot be compiled. The Oracle base image had no `gcc`, no `glibc-devel`, no `clang`, and no kernel headers at all.

*How we faced it:*
We installed the complete C development toolchain via dnf:
```bash
sudo dnf install -y gcc gcc-c++ clang make glibc-devel
```
This gave the system `gcc` and all required C standard library headers. `rusqlite`'s build script could now locate `gcc`, compile `sqlite3.c`, and link the resulting object file into the final Rust binary. After this installation, `rusqlite` compiled successfully in approximately 4 minutes.

-> *Verify: `gcc --version` returns a version string and `cargo build -j 1` proceeds past the `rusqlite` build step.*


**6. Zombie Process RAM Starvation**

*The Pitfall (Deep Context):*
Because builds took 45+ minutes and SSH connections dropped after 5-10 minutes of network inactivity, we would frequently return to a dead terminal. We would reconnect via SSH and start a new `cargo build`. What we did not realize initially was that the previous build was not dead. It was still running as an orphaned process, invisibly consuming RAM and CPU with no terminal attached to it.

When we started the second build on top of the first, two simultaneous `rustc` processes were competing for the same 1GB of RAM. The machine crashed within seconds. We then reconnected and started a third build on top of the first two. The machine crashed immediately. It took us embarrassingly long to realize the pattern: we were crashing the machine by stacking live orphan processes on top of each other.

*How we faced it:*
We added an explicit kill step at the beginning of every deployment script:
```bash
pkill -9 -f 'cargo build' || true
pkill -9 -f 'rustc' || true
sleep 2
```
The `|| true` prevents the script from exiting if no matching process exists. After the kill, we wait 2 seconds for process cleanup before starting fresh. We also switched to running builds with `nohup` and explicit PID files so we could track exactly what was running when we reconnected:
```bash
nohup cargo build --release -j 1 >> build.log 2>&1 &
echo $! > build.pid
```
Checking `cat build.pid` on reconnect tells us immediately whether a build is already running before we start another one.

-> *Verify: `ps aux | grep cargo` returns at most one running cargo process before initiating any new build.*
