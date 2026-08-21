# Compute & Compilation Constraints

**1. OOM Kernel Panics**
*The Pitfall:* When we pivoted to native compilation on the Oracle VM, we completely underestimated how violently the Linux kernel reacts to memory exhaustion. We naively executed a default `cargo build --release` expecting a slow compile. Instead, Cargo aggressively spawned overlapping compiler threads that immediately swallowed the microscopic 1GB of RAM. The server didn't just slow down—it violently locked up as the OOM-killer randomly assassinated system processes before terminating the VM itself.
*How we faced it:* We had to completely abandon multi-threaded compilation. We artificially choked Cargo, forcing it to crawl sequentially across the 150+ crates just to keep the RAM footprint tiny enough to survive. 
-> *Verify: `cargo build -j 1` completes without dropping SSH sessions.*

**2. Link Time Optimization Memory Spikes**
*The Pitfall:* Even with thread strangulation, the final linking phase kept killing the server. Our `Cargo.toml` originally used `lto = true` to yield the absolute fastest runtime binary possible. But Link Time Optimization forces the linker to load the entire compiled dependency graph into RAM at the exact same moment. On a 1GB machine, this physically ripped through the remaining memory headroom, crashing the build at 99%.
*How we faced it:* We had to sacrifice a fraction of runtime performance for bare-metal survival. We ripped `lto = true` out of the manifest entirely, disabling the optimization to keep the final memory spike under 900MB.
-> *Verify: Binary size increases marginally but RAM footprint stays under 900MB during final step.*

**3. Compiler Code Generation Choking**
*The Pitfall:* Cargo's monolithic code generation units were too massive for the tiny CPU cache to hold. The single Oracle CPU core was spending more time swapping cache blocks than actually compiling, causing the build to stall out in an infinite loop of thermal/cache starvation.
*How we faced it:* We brutally split the compile load by forcing the compiler to output in smaller, bite-sized units (`codegen-units = 16`), sacrificing compile-time optimization but allowing the starved CPU cache to actually digest the code blocks cleanly.
-> *Verify: `grep codegen-units Cargo.toml` returns `16`.*

**4. Duplicate Cargo Profiles**
*The Pitfall:* In our frantic desperation to fix the LTO and Codegen panics, we accidentally pasted multiple `[profile.release]` tables into the `Cargo.toml`. When we triggered the build, cargo threw a fatal syntax exception halfway through the dependency initialization, halting our only working build matrix.
*How we faced it:* We performed a surgical merge of the configuration vectors, unifying all the desperate overrides (LTO false, codegen 16) into one clean manifest block.
-> *Verify: `cargo check` parses the manifest securely without throwing table conflict panics.*

**5. C Header Bindgen Failures**
*The Pitfall:* Because we couldn't cross-compile (due to `glibc` linking drops), we expected the native compilation to just work. The absolute nightmare occurred when `rusqlite` hit its C-bindings phase. It threw `stdarg.h not found`. We realized the Oracle Linux minimal bare-bones image had literally zero C-toolchains installed. 
*How we faced it:* We had to manually inject the underlying system C architecture dependencies into the Linux host—`gcc`, `clang`, `llvm-devel`, and `glibc-devel`—to forcefully satisfy the SQLite C-header demands just to compile.
-> *Verify: `gcc --version` returns compiler presence and SQLite finishes build matrix unconditionally.*

**6. Zombie Process Ram Starvation**
*The Pitfall:* During the 20-minute native builds, SSH TCP timeouts inevitably severed our connections. When the terminal disconnected, Linux technically ended the session, but the orphan `rustc` processes became zombies running silently in the background. When we reconnected to start again, the next `cargo build` command stacked on top of the zombies, utilizing all 1GB RAM and instantly exploding the server.
*How we faced it:* We developed a brutal startup kill-switch. Before any new background process could be spawned, we commanded the OS to aggressively hunt and kill any lingering compiler ghosts in the entire process tree.
-> *Verify: `pkill -f 'cargo build'` executes securely matching zero processes prior to execution.*