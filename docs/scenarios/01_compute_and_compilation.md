# Compute & Compilation Constraints

**1. OOM Kernel Panics**
*The Pitfall (Deep Context):* The foundational disaster of this entire deployment was the sheer physical limitations of the Oracle `VM.Standard.E2.1.Micro` tier. We were supposed to be on an Ampere A1 flex shape, which offers 24GB of RAM—more than enough to allow `rustc` to hold its massive Abstract Syntax Trees (AST) in memory. When we lost access to the Ampere cluster due to region exhaustion, we were forced onto a 1GB x86_64 server. 
We severely underestimated what 1GB of RAM means to a modern compiler. We executed a naive `cargo build --release`. Because `cargo` is designed to be highly concurrent, it immediately surveyed the system, detected available thread pooling, and spawned parallel `rustc` processes for the 150+ dependencies required by the Axum and SQLite backends. Within approximately 14 seconds, the memory consumption spiked from 150MB to 980MB. 
At this exact precipice, the Linux kernel entered a state of sheer panic. The Out-Of-Memory (OOM) Killer awoke. Instead of gracefully pausing threads, the OOM-killer identifies massive memory hogs and instantly sends a non-interceptable `SIGKILL` (-9). But because the memory exhaustion occurred so violently, the entire OS networking stack seized. The SSH session completely froze, `htop` refused to respond, and the Oracle OCI dashboard lit up in red, actively showing "VM Terminated" as the kernel forcefully restarted to recover from the freeze. We realized that compiling Rust on a 1GB machine without aggressive throttling was physical suicide for the server.

*How we faced it:* We had to completely discard the dream of a fast compile. We artificially choked Cargo, throwing a choke chain over its concurrent solver. By invoking the process with `-j 1` (Jobs = 1), we forced `cargo` to strictly sequentially compile a single dependency at a time. The CPU usage dropped to a flat 10%, compile times ballooned into eternity, but the memory footprint stayed just under the fatal threshold of 950MB. We sacrificed an hour of compile time to keep the server alive.
-> *Verify: `cargo build -j 1` completes without dropping SSH sessions.*


**2. Link Time Optimization Memory Spikes**
*The Pitfall (Deep Context):* Even after successfully choking the parallelism to 1 job, we hit a devastating wall at the absolute final step of compilation. Rust compiles individual crates into `.rlib` files, and then the linker binds them all together into the final ELF binary. Originally, our `Cargo.toml` utilized `lto = true` (Link Time Optimization) to squeeze out the absolute maximum Requests Per Second (RPS) for the Axum server. 
What we didn't calculate was the mechanical reality of LTO. To perform deep cross-crate optimizations, the linker must load the Intermediate Representation (IR) of *every single dependency* into RAM simultaneously. At compile step `[151/152]`, the RAM usage idled at 300MB, then shot vertically to 2.4GB. Even with our 4GB artificial SSD swapfile active, the violent I/O thrashing to the SSD caused the linker to time out, crashing the build. The kernel didn't OOM, but the linker categorically refused to complete the binary due to memory allocations timing out.

*How we faced it:* We had to negotiate with the metal. We made the painful architectural decision to sacrifice 10-15% of potential runtime CPU throughput just to guarantee the binary could actually be built. We opened the manifest and ripped `lto = true` out, replacing it with `lto = false`. This forced the linker to fall back to simple localized optimizations, never attempting to load the entire monolith into memory. The memory spike clamped down to a manageable 850MB during linking.
-> *Verify: Binary size increases marginally but RAM footprint stays under 900MB during final step.*


**3. Compiler Code Generation Choking**
*The Pitfall (Deep Context):* The torture didn't end with LTO. We noticed that during the compilation of massive macro-heavy libraries like `serde_derive` and `sqlx-macros`, the single overloaded CPU core was stalling. Cargo attempts to split code generation into massive monolithic units (defaulting to 256 `codegen-units` for fast parallel builds). However, trying to process 256 massive code blocks over a choked, single-threaded execution context with incredibly poor SSD I/O led to severe L1/L2 cache thrashing on the Oracle processor. The CPU was spending 90% of its clock cycles swapping cache lines and only 10% actually compiling our code. The build slowed to a virtual halt, taking over 45 minutes just to compile the `regex` crate.

*How we faced it:* We hacked the cargo manifest to fundamentally rethink how it hands code to LLVM. We injected `codegen-units = 16`, severely lowering the amount of parallel code generation blocks LLVM attempts to juggle. By feeding the compiler smaller, more digestible logical chunks, the CPU cache managed to hold the instructions without thrashing. It heavily penalized the theoretical maximum optimization of the binary, but the compile time for macro-heavy dependencies dropped from 45 minutes back down to 4 minutes.
-> *Verify: `grep codegen-units Cargo.toml` returns `16`.*


**4. Duplicate Cargo Profiles**
*The Pitfall (Deep Context):* During the frantic midnight debugging sessions where we were repeatedly modifying `Cargo.toml` to inject `lto = false` and `codegen-units = 16`, human error compounded our misery. In a frantic `vim` session over a lagging SSH connection, we accidentally pasted a duplicate `[profile.release]` block into the bottom of the manifest. We kicked off the 50-minute compile and went to get coffee, expecting a finished binary. 
Instead, we returned to an immediate `error: manifest contains duplicate table [profile.release]`. We had wasted nearly an hour assuming it was building, only to realize it failed on millisecond one. 

*How we faced it:* We initiated a complete surgical scrub of the entire configuration setup. We merged all optimization overrides into a single, aggressively guarded manifest block at the absolute top of the file, completely nullifying any chance of duplicate trailing arrays. We also mandated executing `cargo check` prior to *any* release build to validate syntax instantly.
-> *Verify: `cargo check` parses the manifest securely without throwing table conflict panics.*


**5. C Header Bindgen Failures**
*The Pitfall (Deep Context):* We felt a massive sense of relief when the pure Rust crates started compiling flawlessly sequentially. Then, we hit `rusqlite`. Native SQLite requires C bindings. The compilation immediately threw a fatal error: `fatal error: stdarg.h: No such file or directory`. 
We were utterly baffled. We assumed the Oracle Linux cloud image would ship with a basic build essential toolchain. It did not. The Oracle Base Image is stripped of literally everything to save megabytes. It lacked `gcc`, `clang`, `make`, and all kernel headers. Rust's `cc` crate was blindly trying to invoke a C-compiler that simply didn't exist in the OS path, killing the build in its tracks.

*How we faced it:* We had to pause the entire Rust execution environment and physically bootstrap the C-architecture into the Linux host via `dnf`. We installed massive payloads: `gcc`, `clang`, `llvm-devel`, and `glibc-devel`. This provided the necessary C headers (`stdarg.h`, `<features.h>`) so `rusqlite` could statically link the SQLite logic into our binary.
-> *Verify: `gcc --version` returns compiler presence and SQLite finishes build matrix unconditionally.*


**6. Zombie Process Ram Starvation**
*The Pitfall (Deep Context):* Because the native compile now took up to 45 minutes on a 1-core machine, we frequently walked away from our terminals. Oracle Cloud's virtual network interface (VNIC) has an aggressive, undocumented TCP KeepAlive killer that silently sever idle SSH sessions after 5-10 minutes. When the SSH pipeline broke, standard Linux behavior is to signal a `SIGHUP` to end the process tree. 
However, because we had wrapped the build in various bash subshells and attempts at `nohup` (before perfecting it), the parent terminal died but the `rustc` compiler process became an orphaned zombie. It was effectively blinded, no longer outputting logs anywhere, but still hungrily consuming the precious 1GB of RAM. When we SSH'd back in, we blindly typed `cargo build` again. Two compilers launched, immediately triggering a fatal OOM system crash.

*How we faced it:* We realized we could absolutely trust nothing about the VM's state upon reconnecting. We baked a rigid kill-switch pattern into our deployment workflow. Before executing any compile command, we forcibly invoked a system-wide `pkill -f 'cargo build'` to hunt down and assassinate any lingering, detached compiler threads holding our RAM hostage.
-> *Verify: `pkill -f 'cargo build'` executes securely matching zero processes prior to execution.*