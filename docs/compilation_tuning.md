#  Throttling Rust Compilation for Micro VMs (1GB RAM)

Attempting to run `cargo build --release` on a highly optimized, asynchronous web server requires compiling nearly 150+ interdependent crates. On an Oracle `VM.Standard.E2.1.Micro` instance equipped with only 1 Core and 1 GB of RAM, the default Rust compilation settings will immediately trigger an Out-of-Memory (OOM) kernel panic, effectively killing/crashing the virtual machine.

This document details the exact overrides injected to safely build the payload.

## 1. Disabling Link Time Optimization (LTO)
By default, the Amrita backend may have been configured with `lto = true` or `lto = "fat"` in `Cargo.toml` to extract maximum runtime speed. While excellent for runtime, LTO requires the linker to hold the **entire compiled codebase** in RAM simultaneously just before producing the final binary. On a 1GB machine, this physically cannot happen.

**Fix Applied (`Cargo.toml`):**
```toml
[profile.release]
lto = false
codegen-units = 16
```

## 2. Throttling Concurrency (`-j 1`)
By default, `cargo` aggressively spins up parallel jobs equating to the available system threads. Although the VM has 1 Core, `cargo` context switching while trying to resolve `rustc` trees causes rapid peak RAM pressure.

**Fix Applied (Terminal):**
```bash
cargo build --release --bin server -j 1
```
The `-j 1` flag forces Rust to strictly compile a single dependency at a time sequentially. It slows the compilation speed down, but provides a steady, tiny memory footprint that prevents the compiler from overflowing into the Swap disk and triggering process deaths.

## 3. Disconnecting Terminal SIGHUP (`nohup`)
Native builds on Micro shapes can take 15 to 25 minutes. Standard SSH connections (especially on `opc@`) often suffer from TCP KeepAlive timeouts after 5 minutes of no terminal output. If the SSH connection drops, Linux sends a `SIGHUP` (Hangup Signal) to all active processes, instantly killing the compiler. 

**Fix Applied (Terminal):**
```bash
nohup cargo build --release --bin server -j 1 >> build.log 2>&1 &
```
This detaches the compiler process completely from the SSH session. Even if your internet connection dies, the VM will obediently finish compiling the code completely in the background.
