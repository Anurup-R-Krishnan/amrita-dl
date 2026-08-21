//! db_sync -- Align index.db relative_path values with physical .meta sidecar files.
//!
//! Usage: db_sync [--root <indexed_root>] [--db <db_path>]
//!
//! Reads every .meta file under <indexed_root>, extracts sha256, computes the
//! exact relative PDF path, and updates the matching row in index.db. Retries
//! if the database is locked (busy_timeout 10s). Idempotent.

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let root = PathBuf::from(
        get_arg(&args, "--root").unwrap_or_else(|| {
            "/run/media/anuruprkris/DATA/amrita-exam-papers-indexed".to_string()
        }),
    );
    let db_path = PathBuf::from(
        get_arg(&args, "--db")
            .unwrap_or_else(|| root.join("index.db").to_string_lossy().to_string()),
    );

    println!("[db_sync] root: {}", root.display());
    println!("[db_sync] db:   {}", db_path.display());

    let conn = Connection::open(&db_path)
        .with_context(|| format!("Cannot open {}", db_path.display()))?;
    conn.execute_batch("PRAGMA busy_timeout = 10000; PRAGMA journal_mode = WAL;")?;

    let started = Instant::now();
    let mut updates: Vec<(String, String)> = Vec::new();
    let mut skipped = 0usize;

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "meta").unwrap_or(false))
    {
        let full_path = entry.path();
        let rel_meta = full_path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let rel_pdf = rel_meta.trim_end_matches(".meta").to_string() + ".pdf";

        match read_sha(full_path) {
            Some(sha) => updates.push((rel_pdf, sha)),
            None => {
                skipped += 1;
            }
        }
    }

    println!(
        "[db_sync] Found {} meta files ({} skipped/unreadable). Updating DB...",
        updates.len(),
        skipped
    );

    let tx = conn.unchecked_transaction()?;
    let mut updated = 0usize;
    for (rel_pdf, sha) in &updates {
        let rows = tx.execute(
            "UPDATE papers SET relative_path = ?1 WHERE sha256 = ?2",
            rusqlite::params![rel_pdf, sha],
        )?;
        updated += rows;
    }
    tx.commit()?;

    println!(
        "[db_sync] Done. {} rows updated in {:.1}s.",
        updated,
        started.elapsed().as_secs_f32()
    );
    Ok(())
}

fn get_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}

fn read_sha(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v.get("sha256")?.as_str().map(|s| s.to_string())
}