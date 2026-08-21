//! upload -- Sync indexed PDF files to OCI Object Storage via rclone.
//!
//! Usage: upload [--src <dir>] [--remote <rclone_remote>] [--dry-run]
//!
//! Excludes .meta sidecar files and index.db from the upload.
//! All arguments are passed to rclone via exec array (no shell interpolation),
//! so filenames with spaces, ampersands, and parentheses are handled correctly.

use anyhow::{bail, Context, Result};
use std::process::Command;

const DEFAULT_SRC: &str = "/run/media/anuruprkris/DATA/amrita-exam-papers-indexed";
const DEFAULT_REMOTE: &str = "oracle-amrita-papers:oracle-amrita-bucket";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let src = get_arg(&args, "--src").unwrap_or_else(|| DEFAULT_SRC.to_string());
    let remote = get_arg(&args, "--remote").unwrap_or_else(|| DEFAULT_REMOTE.to_string());
    let dry_run = args.iter().any(|a| a == "--dry-run");

    if !std::path::Path::new(&src).is_dir() {
        bail!("Source directory not found: {src}");
    }

    println!("[upload] Source : {src}");
    println!("[upload] Remote : {remote}");
    if dry_run {
        println!("[upload] Mode   : dry-run (no files will be transferred)");
    }

    let mut cmd = Command::new("rclone");
    cmd.args([
        "sync",
        &src,
        &remote,
        "--exclude",
        "*.meta",
        "--exclude",
        "index.db",
        "--transfers",
        "16",
        "--checkers",
        "32",
        "--checksum",
        "--fast-list",
        "--progress",
    ]);

    if dry_run {
        cmd.arg("--dry-run");
    }

    let status = cmd.status().context("Failed to launch rclone")?;
    if !status.success() {
        bail!("rclone exited with status: {status}");
    }

    println!("[upload] Sync complete.");
    Ok(())
}

fn get_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}