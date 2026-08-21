# Operations & Maintenance Guide

## 1. Local Development (`run_local.sh` deprecated)

The legacy hybrid Bash approach is fully replaced securely. Development now integrates natively:
- The actual server parses natively responding on port `80` (or configured override).
- The database directly targets the single source of truth SQLite file localized in your workspace.
- The Object Storage sync workflows bypass localized Bash wrappers entirely.

## 2. Syncing the Database and Buckets natively in Rust.

Legacy `perfect_sync.py` and `upload_pdfs.py` routines generated database corruption deadlocks and Rclone string splitting failures. The workflow has been definitively rewritten securely:

```bash
# Push schema realignments avoiding thread locking delays
cargo run --release --bin db_sync

# Execute Object Storage sync avoiding Space/Ampersand parsing faults
cargo run --release --bin upload
```

These executables compile against standard target architecture guaranteeing zero reliance on local interpreter parity anomalies.