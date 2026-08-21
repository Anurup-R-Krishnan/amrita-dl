# ️ Operations & Maintenance Guide

## 1. Local Development (`run_local.sh`)

When working locally, you cannot serve the 9.2GB folder from your machine natively if the drive is unplugged. The `scripts/run_local.sh` file mounts a hybrid environment:
- The actual server runs on `localhost:3000`.
- The database points to your local `index.db` copy.
- But `STORAGE_PUBLIC_URL` is hardcoded to Oracle Cloud!

**What this means:** You can develop the UI locally against `http://localhost:3000`. Searching queries your local `index.db`, but clicking "Download PDF" redirectly fetches the PDF from the live cloud bucket, meaning you don't need the 9.2GB hard drive plugged in just to write CSS!

## 2. Pushing Data Updates to Oracle Cloud

If Amrita releases new exams in the future, you will run `amrita-index` to process the PDFs.
Once the `amrita-exam-papers-indexed` folder on your external drive has new files, you sync it to Oracle Cloud natively using the provided shell script.

```bash
# Preview what would be uploaded (Dry Run)
./scripts/sync_to_oracle.sh --dry-run
```

**Under the Hood (`rclone`)**:
- Uses 16 parallel transfers and 32 parallel checkers to maximize bandwidth.
- Strictly ignores `.meta` files using the `--exclude "*.meta"` flag (saving useless thousands of JSON objects from rotting in your bucket).
- Uses `--fast-list` to avoid fetching OCI directories one-by-one, cutting sync time dramatically on the 29,678 objects.

## 3. Database Schema Re-Alignment

If the Rust binary `amrita-index` is modified in the future to change how strings are formatted (slugification), the OCI paths will break. 
You must run the Python alignment script located at `scripts/rebuild_db_paths.py`. It perfectly mirrors the Rust string mutation rules into `index.db`'s `relative_path` column directly in-place without triggering massive data migrations.
