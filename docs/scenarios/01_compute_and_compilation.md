# Compute & Compilation Constraints

**1. OOM Kernel Panics**
- *Roadblock:* Default `cargo build` spawned too many threads, exhausting 1GB RAM instantly causing VM termination.
- *Fix:* Throttled concurrency via single-thread execution wrapper.
-> *Verify: `cargo build -j 1` completes without dropping SSH sessions.*

**2. Link Time Optimization Memory Spikes**
- *Roadblock:* `[profile.release]` contained `lto = true`, forcing the entire binary into RAM during linking blocking successful builds.
- *Fix:* Disabled link time optimization (`lto = false`).
-> *Verify: Binary size increases marginally but RAM footprint stays under 900MB during final step.*

**3. Compiler Code Generation Choking**
- *Roadblock:* Cargo attempted massive monolithic compilation units blocking the CPU cache natively.
- *Fix:* Partitioned load by injecting constrained compilation units.
-> *Verify: `grep codegen-units Cargo.toml` returns `16`.*

**4. Duplicate Cargo Profiles**
- *Roadblock:* Execution explicitly crashed citing conflicting `[profile.release]` tables mid-build.
- *Fix:* Merged duplicate profile parameter definitions inside repository configurations.
-> *Verify: `cargo check` parses the manifest securely without throwing table conflict panics.*

**5. C Header Bindgen Failures**
- *Roadblock:* `rusqlite` bundled compiled halted specifically citing `stdarg.h not found`. Minimum Oracle image lacked C headers.
- *Fix:* Installed missing C-toolchain packages matching underlying compilation architectures.
-> *Verify: `gcc --version` returns compiler presence and SQLite finishes build matrix unconditionally.*

**6. Zombie Process Ram Starvation**
- *Roadblock:* SSH disconnects orphaned active Cargo threads consuming all 1GB RAM and denying restart paths cleanly.
- *Fix:* Injected rigid process execution killing prior to launching new background processes.
-> *Verify: `pkill -f 'cargo build'` executes securely matching zero processes prior to execution.*