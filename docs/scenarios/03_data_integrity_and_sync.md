# Data Integrity & Shell Script Overrides

**14. Legacy Perfect Sync DB Locking**

*The Pitfall (Deep Context):*
Our original strategy for dumping 19,600+ PDF URLs into the database was a Python script cheekily named `perfect_sync.py`. It sounded great in theory: to save time, the script used Python's `concurrent.futures` to blast multi-threaded rows into the SQLite engine from multiple worker threads simultaneously.

We watched the terminal erupt in exceptions. The Python SQLite C bindings do not support arbitrary multi-threaded access without explicit serialization logic. Because the unmanaged thread barrage all tried to grab the main database file handle at once, SQLite panicked to protect the data and threw frantic `database is locked` exceptions globally. Worse than the exceptions was what happened silently: retry logic swallowed some failures, so Python reported success while having dropped roughly 60% of our production rows. A massive data integrity hole sat inside our archive, invisible until someone searched for a missing paper.

*How we faced it:*
We burned the Python bridge entirely. Python lacked the rigid concurrency guarantees we needed without extensive manual locking hacks. We rewrote the entire indexing logic as a native Rust binary (`db_sync.rs`). By leveraging Rust's compile-time concurrency safety alongside strict `rusqlite` serialization (`PRAGMA journal_mode=WAL`, all writes wrapped in single `EXCLUSIVE` transactions), every row committed cleanly in order. Not a single lock was dropped, and 100% of the payload landed.

-> *Verify: `cargo run --bin db_sync` processes the full dataset without `database is locked` errors, and `sqlite3 index.db "SELECT COUNT(*) FROM papers"` matches the source URL count exactly.*


**15. Rclone Shell Injection Vulnerabilities**

*The Pitfall (Deep Context):*
The legacy architecture had a massive security and stability hole buried in `upload_pdfs.py`. The script blindly used Python's `os.system()` to construct raw bash strings, passing hundreds of physical filepath targets directly to `rclone`.

In an academic database, professors name files chaotically. The instant the script hit a university folder titled `2024 & Exams` or a file like `Data Structures.pdf`, the Linux kernel interpreted the uncontrolled string literally. The `&` symbol threw the upload command into the background as an orphaned shell job. Spaces split single paths into multiple broken arguments. `rclone` failed with hundreds of "missing object" errors while our own shell was executing fragments of file names as instructions. We were actively injecting ourselves.

*How we faced it:*
This required surgical refactoring to escape shell interpolation entirely. We tore down the Python wrapper and rewrote the blob uploader as `upload.rs`. Rust's `std::process::Command::arg()` bypasses the bash layer completely: arguments are passed to the `rclone` executable through the kernel's argv array with no string parsing or interpolation. Ampersands, quotes, and spaces are treated as pure string data rather than shell instructions:
```rust
Command::new("rclone")
    .arg("copy")
    .arg(&local_path)
    .arg(format!("oracle:{}{}", remote_bucket, remote_path))
    .status()
```
Every hostile filename now uploads byte-identical to its source path.

-> *Verify: `upload.rs` completes a pass over the full corpus with zero shell errors, and spot-checking a filename containing `&` and spaces shows it stored intact via `rclone lsl oracle:`.*


**16. Verifying Massive Blob Storage Hash Deduplication**

*The Pitfall (Deep Context):*
When we finally piped the rclone stream to Oracle Cloud Object Storage, we pushed 64,600 individual file blobs over terrible home-network channels. Once the terminal completed the final loop, rclone simply returned an empty prompt.

Paranoia set in immediately. Did the bucket actually receive 100% of the payload? Were all 9+ GB transferred without mid-stream drops? Were there empty zero-byte allocations where a PDF should be? We could not trust a silent prompt. We had to prove empirically that the physical objects in the remote cloud matched the local FTS SQLite tracking index, otherwise any student clicking a search result would get a broken download.

*How we faced it:*
We engineered a network-bound validation loop instead of manually inspecting buckets in the GUI. Running `rclone size oracle:bucket-name` fired recursive metadata checks against the remote structure, fetching only byte counts and object counts without pulling the files themselves. We compared those numbers directly against `SELECT COUNT(*), SUM(size_bytes)` from the local tracking index. Both totals matched exactly: 64,600 objects, identical byte sums. Only then did we consider the migration complete.

-> *Verify: `rclone size oracle:<bucket>` output registers exactly the same count and total bytes as the FTS index query before declaring the sync done.*


**17. SQLite Index Portability Risks**

*The Pitfall (Deep Context):*
Because native building on the Oracle VM kept failing early on, we tried building the 13MB `index.db` completely locally on an Apple Silicon MacBook and shipping it to the Linux target over the network.

It ended in absolute data corruption. Modern SQLite uses a Write-Ahead Log (`.db-wal` and `.db-shm` sidecar files) for high-speed indexing. During the macOS-to-Linux transfer, the copy tooling cloned the live `.wal` state alongside the database. Because the WAL contained uncommitted page state from the Mac session, the remote Linux SQLite binary refused to mount the index coherently. The database arrived corrupted, locking us out of the entire archive on deployment day.

*How we faced it:*
We stripped away all clever abstractions: no network volumes, no live copies, no sidecar files. We reverted to brute-force explicit syncing. Before every transfer we run `PRAGMA wal_checkpoint(TRUNCATE);` followed by closing the connection, then transfer strictly the flat `index.db` alone via `scp`. Excluding running WAL state guarantees the destination receives a self-contained, fully-committed snapshot that mounts identically on any architecture:
```bash
sqlite3 index.db "PRAGMA wal_checkpoint(TRUNCATE);" && \
scp index.db opc@vm:~/data/index.db
```

-> *Verify: On the VM, `sqlite3 index.db "PRAGMA integrity_check"` yields `ok`.*


**18. Residual Python Bloat**

*The Pitfall (Deep Context):*
As we deployed our native Rust logic, CI pipelines started failing randomly on steps that had nothing to do with our code, throwing confusing environment errors referencing missing `python3-pip` libraries.

The cause was embarrassing: we had stripped Python from the Oracle OS base image to save memory, but the repository still harbored dead legacy Python files we thought we had abandoned weeks earlier. GitHub Actions' auto-detection picked up on these artifacts, interpreted the repository loosely as a Python project, and attempted to validate non-functional Python dependencies, halting our real Rust pipeline instantly. Ghost code was ghosting our builds.

*How we faced it:*
We initiated a ruthless git scrub. We invoked `git rm` across every unused Python and bash validation artifact left behind, then verified nothing referenced them:
```bash
git rm scripts/*.py legacy/*.py
git commit -m "chore: remove dead python artifacts confusing CI detection"
```
By standardizing the repository structure as pure monolithic Rust, CI parsers lock onto Cargo and stop hallucinating dependency mappings that never existed.

-> *Verify: `find . -name "*.py" -not -path "./node_modules/*"` returns nothing, and the next GitHub Actions run selects the Rust toolchain without pip errors.*
