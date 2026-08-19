use anyhow::{Context, Result};
use clap::Parser;
use rayon::prelude::*;
use rusqlite::Connection;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about = "High-performance Rust physical verification audit tool for Amrita PDF dataset")]
struct Args {
    #[arg(
        long,
        default_value = "/run/media/anuruprkris/DATA/amrita-exam-papers"
    )]
    raw_dir: PathBuf,

    #[arg(
        long,
        default_value = "/run/media/anuruprkris/DATA/amrita-exam-papers-indexed"
    )]
    indexed_dir: PathBuf,

    #[arg(long, default_value = "dataset_verification_report.txt")]
    output_report: PathBuf,

    #[arg(long, default_value_t = false)]
    allow_zero_files: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexedFileStats {
    pub total_files: u64,
    pub total_bytes: u64,
    pub valid_headers: u64,
    pub invalid_headers: u64,
    pub zero_bytes: u64,
    pub traversal_errors: u64,
    pub metadata_errors: u64,
    pub read_errors: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let start_time = Instant::now();

    println!("=================================================================");
    println!("      AMRITA-DL PDF DATASET RUST PHYSICAL INTEGRITY AUDIT        ");
    println!("=================================================================");

    // 1. Audit Raw Directory
    let (raw_count, raw_bytes, raw_inodes) = if args.raw_dir.exists() {
        audit_raw_directory(&args.raw_dir)?
    } else {
        println!("[WARN] Raw directory {} not found", args.raw_dir.display());
        (0, 0, 0)
    };

    println!("[RAW DIRECTORY]");
    println!("  - Directory Path: {}", args.raw_dir.display());
    println!("  - Total PDF Files: {}", raw_count);
    println!(
        "  - Total Size: {} bytes ({:.3} GiB / {:.3} GB)",
        raw_bytes,
        raw_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        raw_bytes as f64 / 1_000_000_000.0
    );
    println!("  - Unique Physical Inodes: {}", raw_inodes);

    // 2. Audit Indexed Directory
    let indexed_stats = audit_indexed_directory(&args.indexed_dir)?;

    let header_pct = if indexed_stats.total_files > 0 {
        100.0 * indexed_stats.valid_headers as f64 / indexed_stats.total_files as f64
    } else {
        0.0
    };

    println!("\n[INDEXED DIRECTORY]");
    println!("  - Directory Path: {}", args.indexed_dir.display());
    println!("  - Total PDF Files: {}", indexed_stats.total_files);
    println!(
        "  - Total Size: {} bytes ({:.3} GiB / {:.3} GB)",
        indexed_stats.total_bytes,
        indexed_stats.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        indexed_stats.total_bytes as f64 / 1_000_000_000.0
    );
    println!(
        "  - Valid '%PDF-' Magic Headers: {} ({:.1}%)",
        indexed_stats.valid_headers, header_pct
    );
    println!(
        "  - Invalid / Corrupted Headers: {}",
        indexed_stats.invalid_headers
    );
    println!("  - Zero-Byte Empty Files: {}", indexed_stats.zero_bytes);
    println!("  - Filesystem Traversal Errors: {}", indexed_stats.traversal_errors);
    println!("  - File Metadata Errors: {}", indexed_stats.metadata_errors);
    println!("  - File Read Errors: {}", indexed_stats.read_errors);

    // 3. Reconcile SQLite Index (index.db)
    let db_path = args.indexed_dir.join("index.db");
    let (db_total, db_matched, db_missing) = reconcile_sqlite(&db_path, &args.raw_dir, &args.indexed_dir)?;

    println!("\n[SQLITE INDEX.DB RECONCILIATION]");
    println!("  - Database Path: {}", db_path.display());
    println!("  - Total Database Records: {}", db_total);
    println!("  - Physically Verified On Disk: {}", db_matched);
    println!("  - Missing Physical Files: {}", db_missing);

    // 4. Reconcile JSON Dump (index.json)
    let json_path = args.indexed_dir.join("index.json");
    let (json_total, json_matched, json_missing) = reconcile_json(&json_path, &args.raw_dir, &args.indexed_dir)?;

    println!("\n[INDEX.JSON RECONCILIATION]");
    println!("  - JSON Dump Path: {}", json_path.display());
    println!("  - Total JSON Records: {}", json_total);
    println!("  - Physically Verified On Disk: {}", json_matched);
    println!("  - Missing Physical Files: {}", json_missing);

    let zero_files_ok = indexed_stats.total_files > 0 || args.allow_zero_files;
    let all_ok = args.indexed_dir.exists()
        && db_path.exists()
        && json_path.exists()
        && zero_files_ok
        && indexed_stats.invalid_headers == 0
        && indexed_stats.zero_bytes == 0
        && indexed_stats.traversal_errors == 0
        && indexed_stats.metadata_errors == 0
        && indexed_stats.read_errors == 0
        && db_missing == 0
        && json_missing == 0;

    let status_str = if all_ok {
        "PASSED SUCCESSFULLY"
    } else {
        "FAILED - SEE COUNTS ABOVE"
    };

    let elapsed = start_time.elapsed();
    println!("\nAudit execution completed in {:.2?}", elapsed);
    println!("AUDIT STATUS: {} ({:.1}% VERIFIED)", status_str, header_pct);

    // Generate Final Report Content
    let report_content = format!(
        "=================================================================\n\
               AMRITA-DL PDF DATASET RUST PHYSICAL INTEGRITY AUDIT REPORT\n\
         =================================================================\n\
         Execution Duration: {:.2?}\n\n\
         Raw Source Directory: {}\n\
         Raw PDF File Count: {}\n\
         Raw Total Size: {} bytes ({:.3} GiB / {:.3} GB)\n\
         Raw Unique Inodes: {}\n\n\
         Indexed Serving Directory: {}\n\
         Indexed PDF File Count: {}\n\
         Indexed Total Size: {} bytes ({:.3} GiB / {:.3} GB)\n\
         Valid '%PDF-' Magic Headers: {} / {} ({:.1}%)\n\
         Invalid Headers / Corrupted: {}\n\
         Zero-Byte Files: {}\n\
         Traversal Errors: {}\n\
         Metadata Errors: {}\n\
         Read Errors: {}\n\n\
         SQLite Database (index.db):\n\
         Total Rows: {}\n\
         Matched Physical Files: {}\n\
         Missing Physical Files: {}\n\n\
         JSON Metadata Dump (index.json):\n\
         Total Entries: {}\n\
         Matched Physical Files: {}\n\
         Missing Physical Files: {}\n\n\
         AUDIT STATUS: {} - {:.1}% OF ALL {} PDF FILES VERIFIED\n\
         =================================================================\n",
        elapsed,
        args.raw_dir.display(),
        raw_count,
        raw_bytes,
        raw_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        raw_bytes as f64 / 1_000_000_000.0,
        raw_inodes,
        args.indexed_dir.display(),
        indexed_stats.total_files,
        indexed_stats.total_bytes,
        indexed_stats.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        indexed_stats.total_bytes as f64 / 1_000_000_000.0,
        indexed_stats.valid_headers,
        indexed_stats.total_files,
        header_pct,
        indexed_stats.invalid_headers,
        indexed_stats.zero_bytes,
        indexed_stats.traversal_errors,
        indexed_stats.metadata_errors,
        indexed_stats.read_errors,
        db_total,
        db_matched,
        db_missing,
        json_total,
        json_matched,
        json_missing,
        status_str,
        header_pct,
        indexed_stats.total_files,
    );

    fs::write(&args.output_report, &report_content)
        .with_context(|| format!("Failed to write report to {}", args.output_report.display()))?;

    println!("\n[OK] Complete audit report written to {}", args.output_report.display());

    if !all_ok {
        anyhow::bail!("Audit failed: physical dataset verification checks did not pass");
    }

    Ok(())
}

fn audit_raw_directory(dir: &Path) -> Result<(u64, u64, usize)> {
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut inodes = HashSet::new();

    for entry in WalkDir::new(dir).into_iter().flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
        {
            count += 1;
            if let Ok(meta) = entry.metadata() {
                bytes += meta.len();
                inodes.insert(meta.ino());
            }
        }
    }

    Ok((count, bytes, inodes.len()))
}

fn audit_indexed_directory(dir: &Path) -> Result<IndexedFileStats> {
    if !dir.exists() {
        return Ok(IndexedFileStats {
            traversal_errors: 1,
            ..Default::default()
        });
    }

    let traversal_errors = AtomicU64::new(0);
    let metadata_errors = AtomicU64::new(0);
    let read_errors = AtomicU64::new(0);

    let pdf_entries: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) => Some(entry),
            Err(_) => {
                traversal_errors.fetch_add(1, Ordering::Relaxed);
                None
            }
        })
        .filter(|e| {
            e.path().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total_files = pdf_entries.len() as u64;
    let total_bytes = AtomicU64::new(0);
    let valid_headers = AtomicU64::new(0);
    let invalid_headers = AtomicU64::new(0);
    let zero_bytes = AtomicU64::new(0);

    pdf_entries.par_iter().for_each(|path| {
        match fs::metadata(path) {
            Ok(meta) => {
                let len = meta.len();
                total_bytes.fetch_add(len, Ordering::Relaxed);
                if len == 0 {
                    zero_bytes.fetch_add(1, Ordering::Relaxed);
                    return;
                }

                let mut header = [0u8; 5];
                match File::open(path) {
                    Ok(mut f) => {
                        if f.read_exact(&mut header).is_ok() && &header == b"%PDF-" {
                            valid_headers.fetch_add(1, Ordering::Relaxed);
                        } else {
                            invalid_headers.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        read_errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Err(_) => {
                metadata_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    Ok(IndexedFileStats {
        total_files,
        total_bytes: total_bytes.load(Ordering::Relaxed),
        valid_headers: valid_headers.load(Ordering::Relaxed),
        invalid_headers: invalid_headers.load(Ordering::Relaxed),
        zero_bytes: zero_bytes.load(Ordering::Relaxed),
        traversal_errors: traversal_errors.load(Ordering::Relaxed),
        metadata_errors: metadata_errors.load(Ordering::Relaxed),
        read_errors: read_errors.load(Ordering::Relaxed),
    })
}

fn reconcile_sqlite(db_path: &Path, raw_dir: &Path, indexed_dir: &Path) -> Result<(usize, usize, usize)> {
    if !db_path.exists() {
        return Ok((0, 0, 1));
    }

    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;

    let mut stmt = conn.prepare("SELECT original_path FROM papers")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

    let mut total = 0;
    let mut matched = 0;
    let mut missing = 0;

    for orig_path in rows.flatten() {
        total += 1;
        let orig_p = Path::new(&orig_path);
        if orig_p.exists() {
            matched += 1;
        } else {
            let rel = if let Ok(stripped) = orig_p.strip_prefix(raw_dir) {
                stripped.to_path_buf()
            } else if let Ok(stripped) = orig_p.strip_prefix(indexed_dir) {
                stripped.to_path_buf()
            } else if let Some(pos) = orig_path.find("Examination  Papers/") {
                PathBuf::from(&orig_path[pos..])
            } else if let Some(pos) = orig_path.find("Examination Papers/") {
                PathBuf::from(&orig_path[pos..])
            } else {
                PathBuf::from(orig_path.trim_start_matches('/'))
            };

            if raw_dir.join(&rel).exists() || indexed_dir.join(&rel).exists() {
                matched += 1;
            } else {
                missing += 1;
            }
        }
    }

    Ok((total, matched, missing))
}

fn reconcile_json(json_path: &Path, raw_dir: &Path, indexed_dir: &Path) -> Result<(usize, usize, usize)> {
    if !json_path.exists() {
        return Ok((0, 0, 1));
    }

    let content = fs::read_to_string(json_path)
        .with_context(|| format!("Failed to read JSON index file at {}", json_path.display()))?;

    let records: Vec<serde_json::Value> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON index at {}", json_path.display()))?;

    let mut total = 0;
    let mut matched = 0;
    let mut missing = 0;

    for rec in records {
        total += 1;
        if let Some(orig_path) = rec.get("original_path").and_then(|v| v.as_str()) {
            let orig_p = Path::new(orig_path);
            if orig_p.exists() {
                matched += 1;
            } else {
                let rel = if let Ok(stripped) = orig_p.strip_prefix(raw_dir) {
                    stripped.to_path_buf()
                } else if let Ok(stripped) = orig_p.strip_prefix(indexed_dir) {
                    stripped.to_path_buf()
                } else if let Some(pos) = orig_path.find("Examination  Papers/") {
                    PathBuf::from(&orig_path[pos..])
                } else if let Some(pos) = orig_path.find("Examination Papers/") {
                    PathBuf::from(&orig_path[pos..])
                } else {
                    PathBuf::from(orig_path.trim_start_matches('/'))
                };

                if raw_dir.join(&rel).exists() || indexed_dir.join(&rel).exists() {
                    matched += 1;
                } else {
                    missing += 1;
                }
            }
        }
    }

    Ok((total, matched, missing))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("amrita_audit_test_{}_{}", name, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let _ = fs::create_dir_all(&path);
        path
    }

    #[test]
    fn test_audit_missing_directory() {
        let missing = PathBuf::from("/nonexistent/indexed/directory/path");
        let stats = audit_indexed_directory(&missing).unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.traversal_errors, 1);
    }

    #[test]
    fn test_audit_missing_db() {
        let dir = create_temp_test_dir("missing_db");
        let raw = dir.join("raw");
        let indexed = dir.join("indexed");
        let missing_db = indexed.join("index.db");

        let (total, matched, missing) = reconcile_sqlite(&missing_db, &raw, &indexed).unwrap();
        assert_eq!(total, 0);
        assert_eq!(matched, 0);
        assert_eq!(missing, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_audit_invalid_headers() {
        let dir = create_temp_test_dir("invalid_headers");
        let indexed = dir.join("indexed");
        fs::create_dir_all(&indexed).unwrap();

        // 1. Valid PDF file
        let valid_pdf = indexed.join("valid.pdf");
        fs::write(&valid_pdf, b"%PDF-1.7 header content").unwrap();

        // 2. Invalid header file
        let invalid_pdf = indexed.join("corrupt.pdf");
        fs::write(&invalid_pdf, b"NOT A PDF HEADER").unwrap();

        // 3. Zero byte file
        let zero_pdf = indexed.join("empty.pdf");
        fs::write(&zero_pdf, b"").unwrap();

        let stats = audit_indexed_directory(&indexed).unwrap();
        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.valid_headers, 1);
        assert_eq!(stats.invalid_headers, 1);
        assert_eq!(stats.zero_bytes, 1);
        assert_eq!(stats.traversal_errors, 0);
        assert_eq!(stats.metadata_errors, 0);
        assert_eq!(stats.read_errors, 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_audit_traversal_errors() {
        let missing_path = PathBuf::from("/nonexistent/path/traversal");
        let stats = audit_indexed_directory(&missing_path).unwrap();
        assert!(stats.traversal_errors > 0);
    }
}
