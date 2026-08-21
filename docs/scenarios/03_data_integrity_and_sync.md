# Data Integrity & Shell Script Overrides

**14. Legacy Perfect Sync DB Locking**
- *Roadblock:* `perfect_sync.py` failed during multi-threaded sequential writes throwing OS-level "database is locked" errors globally.
- *Fix:* Rewrote integration into `db_sync.rs` leveraging specific `rusqlite` pragmas (`pragma journal_mode=WAL`) ensuring lock bypass executions.
-> *Verify: `cargo run --bin db_sync` processes full dataset without explicit lockpanics matching DB state unconditionally.*

**15. Rclone Shell Injection Vulnerabilities**
- *Roadblock:* Shell execution parsed `upload_pdfs.py` aborting upon identifying ampersand `&` spaces mapping broken Object Storage domains.
- *Fix:* Rewrote to `upload.rs` using `std::process::Command::arg()` bypassing native shell parameter string interpolations.
-> *Verify: `upload.rs` outputs Object Storage mappings dynamically tracking complex file structures securely.*

**16. Verifying Massive Blob Storage Hash Deduplication**
- *Roadblock:* Speculation regarding upload integrity against empty object allocations generated redundancy uploading sequences. 
- *Fix:* Utilized explicit `rclone size` across the Oracle target verifying byte arrays matched indexing constraints identically.
-> *Verify: Output registers `Total objects: 64601` explicitly validating remote buckets against static SQL indices.*

**17. SQLite Index Portability Risks**
- *Roadblock:* Syncing 13MB `index.db` by cloning local states via network volumes corrupted write-ahead log parameters.
- *Fix:* Enacted direct SCP transfers transferring static state models avoiding read-level disruptions via volume wrappers.
-> *Verify: `sqlite3 index.db "PRAGMA integrity_check"` yields `ok` over remote systems completely mapping intact parameters.*

**18. Residual Python Bloat**
- *Roadblock:* CI/CD systems interpreted legacy Python logic crashing builds during environmental validation flows.
- *Fix:* Invoked `git rm` deleting every outdated bash validation execution universally standardizing strictly pure Rust deployments.
-> *Verify: `ls scripts/` restricts executions explicitly removing non-native dependency triggers avoiding CI pipeline confusions.*