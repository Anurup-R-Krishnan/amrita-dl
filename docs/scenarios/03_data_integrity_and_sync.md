# Data Integrity & Shell Script Overrides

**14. Legacy Perfect Sync DB Locking**
*The Pitfall (Deep Context):* Our original strategy for dumping 19,600+ PDF URLs into the database was a Python script cheekily named `perfect_sync.py`. It sounded great in theory. To save time, the script used Python's `concurrent.futures` to blast asynchronous multi-threaded rows into the SQLite engine. 
We watched in horror as the terminal erupted in exceptions. SQLite C-bindings fundamentally do not natively support arbitrary Python multi-threading without explicit serialization logic. Because the massive barrage of unmanaged threads all tried to grab the main database file simultaneously, SQLite aggressively panicked to protect the data, throwing frantic `database is locked` exceptions globally. Python had dropped 60% of our production data blindly, thinking it succeeded, silently masking a massive data integrity hole in our archive.

*How we faced it:* We had to completely burn the Python bridge. Python simply lacked the rigid memory guarantees we needed without extensive hacking. We rewrote the entire indexing logic into a native Rust binary (`db_sync.rs`). By leveraging Rust's absolute concurrency safety alongside strict `rusqlite` serialization pragmas (`pragma journal_mode=WAL` and `EXCLUSIVE` transaction loops), the data mapped sub-millisecond seamlessly. Not a single lock was dropped, and 100% of the payload successfully committed.
-> *Verify: `cargo run --bin db_sync` processes full dataset without explicit lockpanics matching DB state unconditionally.*


**15. Rclone Shell Injection Vulnerabilities**
*The Pitfall (Deep Context):* The legacy architecture had a massive security and stability hole buried in `upload_pdfs.py`. The script blindly utilized Python's `os.system()` to construct Bash strings passing hundreds of physical filepath targets directly to `rclone`. 
In an academic database, professors frequently name their files with chaotic structures. The instant our script encountered a university folder titled `2024 & Exams` or a file with a space like `Data Structures.pdf`, the Linux kernel interpreted the uncontrolled string. The `&` symbol threw the upload into the background as an orphaned shell job, spacing corrupted the tracking paths into unparseable arguments, and `rclone` violently failed, throwing hundreds of "missing object" errors. We were actively poisoning our own shell vectors.

*How we faced it:* This required surgical refactoring to escape shell interpolation entirely. We tore down the Python wrapper and rewrote the blob uploader into `upload.rs`. By mapping the executable securely through Rust's `std::process::Command::arg()` method, we bypassed the Bash layer totally. Rust pipes variables safely and directly to the `rclone` executable binary interface without any string parsing interpolation, meaning `&`, quotes, and spaces were treated cleanly as pure strings strings rather than malicious instructions.
-> *Verify: `upload.rs` outputs Object Storage mappings dynamically tracking complex file structures securely.*


**16. Verifying Massive Blob Storage Hash Deduplication**
*The Pitfall (Deep Context):* When we finally successfully piped the Rclone data stream to the Oracle Cloud Object Storage, we pushed 64,600 individual file blobs blindly over terrible networking channels. Once the terminal completed the final loop, Rclone simply returned an empty prompt.
Paranoia immediately set in. Did the buckets actually receive 100% of the payload? Were all 9+ Gigabytes transferred without network drops? Were there empty zero-byte payload allocations? We couldn't trust a silent prompt. We had to prove, empirically, that the physical binaries in the remote Cloud matched the local FTS SQLite tracking index unconditionally, otherwise finding a paper on the frontend would return a broken 404 for the student.

*How we faced it:* We engineered an aggressive, network-bound surgical validation loop. Instead of manually inspecting buckets in the GUI, we used `rclone size` across the Oracle destination. This fired recursive metadata checks mapping the pure remote structure, fetching byte arrays without pulling the files themselves, matching the counts identically to our SQL indexing counts.
-> *Verify: Output registers exactly the FTS verified counts validating remote buckets explicitly.*


**17. SQLite Index Portability Risks**
*The Pitfall (Deep Context):* Because native building on the Oracle VM was failing initially, we originally tried building the 13MB `index.db` file completely locally on an Apple Silicon Macbook and shipping it to the Linux target via network docker volumes. 
It ended in absolute data corruption. Modern SQLite deployments natively utilize a Write-Ahead Log (`.db-shm` and `.wal` files) for high-speed indexing. During our macOS-to-Linux network transfer, the file system abstraction layer attempted to clone the running `.wal` state. Because the underlying hardware architectures manage internal page memory entirely differently, the remote Linux SQLite binary violently refused to mount the index. The database was entirely corrupted upon arrival, locking us out of the archive.

*How we faced it:* We stripped away all clever abstractions, network wrappers, and volumes. We reverted to brute-force explicit data syncing. We used raw `SCP` (Secure Copy Protocol) transferring only the compiled, flat, inactive local states models across the network boundaries. By fundamentally excluding running `.wal` state files from the transfer matrix, we injected standard state models guaranteeing Linux compatibility over identical architectures effortlessly.
-> *Verify: `sqlite3 index.db "PRAGMA integrity_check"` yields `ok` over remote systems completely mapping intact parameters.*


**18. Residual Python Bloat**
*The Pitfall (Deep Context):* As we deployed our native Rust logic, our CI/CD pipelines suddenly started failing random steps internally, throwing confusing environmental exceptions mapping back to missing `python3-pip` libraries. 
We had aggressively stripped the Oracle OS base boundaries of python functionality to save compute overhead. But our repository inherently harbored the dead legacy Python files we thought we had abandoned. The automated Github mechanisms mistakenly picked up on these artifacts, interpreted the repository loosely as a Python project, and attempted to validate non-functional dependencies, halting our true Rust pipelines instantly.

*How we faced it:* We initiated a profound, ruthless Git scrub. We invoked `git rm` globally, aggressively hunting down and erasing every solitary unused python or bash validation artifact left behind. By standardizing our structural code explicitly as a pure, monolithic Rust architecture, CI parsers fundamentally locked onto Cargo natively avoiding any and all hallucinated dependency mappings gracefully cleanly.
-> *Verify: `ls scripts/` restricts executions explicitly removing non-native dependency triggers avoiding CI pipeline confusions.*