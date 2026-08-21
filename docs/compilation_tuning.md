# Throttling Rust Compilation for Micro VMs (1GB RAM)

## The CachyOS Cross-Compile Nightmare
Initially, avoiding native compilation on a starved 1GB VM was the obvious choice. The plan was to cross-compile the binary locally on a high-powered Arch (CachyOS) workstation and simply `scp` the optimized executable to the server. But deploying this optimized x86_64 binary immediately resulted in systemd throwing fatal `(code=exited, status=203/EXEC)` errors. 

We had slammed straight into an obscurity linking panic. Compiling C-dependencies like `rusqlite` against Arch's bleeding-edge `glibc` headers meant the binary was completely incompatible with the older Oracle Linux kernel. The binary was fundamentally rejected by the OS. Cross-compilation was dead. 

We were forced to directly pull the source code via `source.tar.gz` and natively compile it on the Oracle `VM.Standard.E2.1.Micro` instance equipped with only 1 Core and 1GB of RAM. The result? Attempting to run standard `cargo build --release` on 150+ interdependent crates immediately triggered severe Out-of-Memory (OOM) kernel panics. The VM violently crashed within seconds as `rustc` starved the system.

This document details the exact overrides injected to safely build the payload under extreme compute exhaustion.

## 1. Disabling Link Time Optimization (LTO)
By default, the Amrita backend was configured with `lto = true` to extract maximum runtime speed. While excellent for runtime throughput, LTO requires the linker to hold the **entire compiled codebase** in RAM simultaneously just before producing the final binary. On a 1GB machine, this physically cannot happen without guaranteed kernel panics.

**Fix Applied (`Cargo.toml`):**
```toml
[profile.release]
lto = false
codegen-units = 16
```
-> *Verify: `grep -E "lto|codegen-units" Cargo.toml` outputs `lto = false` and `codegen-units = 16` ensuring compiler strangulation is active.*

## 2. Throttling Concurrency (`-j 1`)
Even with limits, `cargo` aggressively spins up parallel jobs equating to the available system threads. Although the VM only has 1 Core, `cargo` aggressively context switching while tracing `rustc` dependency trees causes rapid peak RAM pressure that spikes straight through our 4GB swap space.

**Fix Applied (Terminal):**
```bash
cargo build --release --bin server -j 1
```
-> *Verify: Execution logs confirm sequential builds by monitoring process states; `top` registers a maximum of one `rustc` thread executing.*

The `-j 1` flag forces Rust to strictly compile a single dependency at a time sequentially. It slows the compilation speed down to an agonizing crawl, but provides a steady, tiny memory footprint that prevents the compiler from overflowing the Swap disk and triggering process deaths.

## 3. Disconnecting Terminal SIGHUP (`nohup`)
Native builds on Micro shapes using restricted 1-core limits take agonizingly long (15 to 25 minutes minimum). Standard SSH connections (especially on Oracle `opc@` accounts) inherently suffer from TCP KeepAlive timeouts after 5 minutes of no terminal output. If the SSH connection dropped, Linux sent a `SIGHUP` (Hangup Signal) to all active processes, instantly assassinating our active compiler session midway through building.

**Fix Applied (Terminal):**
```bash
nohup cargo build --release --bin server -j 1 >> build.log 2>&1 &
```
-> *Verify: `cat build.pid` or `jobs` confirms the detached background process is actively executing avoiding TCP breaks.*

This detaches the compiler process completely from the SSH session. Even if the internet connection dies and the SSH shell breaks, the VM obediently finishes compiling the database codebase in the blind background.