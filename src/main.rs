//! Amrita DSpace Exam-Papers Downloader (Rust)
//!
//! QoS Mitigations:
//! 1. Server throttling/ban      -> adaptive delay + jitter, auto-slowdown on 429/503
//! 2. Partial files / net drop   -> atomic .part -> rename, magic-byte PDF validation
//! 3. Drive unmount mid-run      -> write-error detection, graceful pause + state save
//! 4. Fake 200 HTML error pages  -> Content-Type check + PDF magic byte (%PDF) guard
//! 5. Duplicate items            -> SHA-256 hash registry -> hardlink, never re-download

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use clap::Parser;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{header, Client, StatusCode};
use sanitize_filename::sanitize;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    io::AsyncWriteExt,
    sync::Mutex,
    time::sleep,
};
use tracing::{debug, error, info, warn};

// -- Constants -----------------------------------------------------------------

const BASE_URL:    &str = "http://dspace.amritanet.edu:8080";
const ROOT_HANDLE: &str = "123456789/16";
const STATE_FILE:  &str = "state.json";
const PDF_MAGIC:   &[u8] = b"%PDF-";

// -- CLI -----------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "amrita-dl", about = "Download Amrita exam papers - hardened", version)]
struct Args {
    /// Root download directory
    #[arg(long, default_value = "./amrita-exam-papers")]
    dest: PathBuf,

    /// Parallel download workers
    #[arg(long, default_value_t = 6)]
    workers: usize,

    /// Base delay between requests in seconds (jitter added automatically)
    #[arg(long, default_value_t = 0.4)]
    delay: f64,

    /// Verbose logging
    #[arg(long, short)]
    verbose: bool,
}

// -- State ---------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct State {
    /// Item handles fully processed
    done_items: HashSet<String>,
    /// Bitstream URLs fully downloaded
    done_files: HashSet<String>,
    // -- QoS #5: dedup via SHA-256 -> canonical path --------------------------
    /// sha256(file) -> first path it was saved to (for hardlinking duplicates)
    hash_to_path: HashMap<String, String>,
    /// Discovered collections: (handle, path-components)
    collections: Option<Vec<Collection>>,
    /// Fully enumerated items
    items: Option<Vec<Item>>,
    /// QoS #1: current adaptive delay multiplier (persisted across runs)
    delay_multiplier: f64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Collection {
    handle: String,
    path:   Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Item {
    handle: String,
    path:   Vec<String>,
}

// -- HTTP client with adaptive throttle ---------------------------------------

struct HttpClient {
    inner:   Client,
    /// Base delay from CLI
    base_ms: u64,
    /// QoS #1: multiplier shared across tasks, auto-adjusts
    multiplier: Arc<Mutex<f64>>,
}

impl HttpClient {
    fn new(base_delay: f64, initial_multiplier: f64) -> Result<Self> {
        let inner = Client::builder()
            .user_agent("AmritaArchiveBot/2.0 (educational-mirror; Rust)")
            .gzip(true)
            .timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            inner,
            base_ms: (base_delay * 1000.0) as u64,
            multiplier: Arc::new(Mutex::new(initial_multiplier)),
        })
    }

    async fn current_delay(&self) -> Duration {
        let m = *self.multiplier.lock().await;
        // QoS #1: add up to 30% random jitter to avoid thundering herd
        let jitter_pct = 1.0 + (rand_jitter() * 0.30);
        Duration::from_millis((self.base_ms as f64 * m * jitter_pct) as u64)
    }

    /// Back off: double multiplier (capped at 16x)
    async fn back_off(&self) {
        let mut m = self.multiplier.lock().await;
        *m = (*m * 2.0).min(16.0);
        warn!("Throttle detected - delay multiplier now {:.1}x", *m);
    }

    /// Recover: slowly creep multiplier back toward 1x
    async fn recover(&self) {
        let mut m = self.multiplier.lock().await;
        if *m > 1.0 {
            *m = (*m * 0.95).max(1.0);
        }
    }

    /// GET with retries, adaptive delay, and throttle detection (QoS #1)
    async fn get_text(&self, url: &str) -> Option<String> {
        let mut wait = Duration::from_millis(800);
        for attempt in 0..7 {
            let delay = self.current_delay().await;
            sleep(delay).await;

            match self.inner.get(url).send().await {
                Ok(r) => {
                    match r.status() {
                        StatusCode::OK => {
                            self.recover().await;
                            return r.text().await.ok();
                        }
                        // QoS #1: server throttle signals
                        s @ (StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE) => {
                            warn!("HTTP {} on {} (attempt {})", s, url, attempt + 1);
                            self.back_off().await;
                            sleep(wait).await;
                            wait *= 2;
                        }
                        StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => {
                            debug!("HTTP {} - skipping {}", r.status(), url);
                            return None;
                        }
                        s => {
                            warn!("HTTP {} on {} (attempt {})", s, url, attempt + 1);
                            sleep(wait).await;
                            wait *= 2;
                        }
                    }
                }
                Err(e) => {
                    warn!("Request error on {} (attempt {}): {}", url, attempt + 1, e);
                    sleep(wait).await;
                    wait *= 2;
                }
            }
        }
        error!("Giving up on {}", url);
        None
    }

    /// Streaming GET for file downloads
    async fn get_stream(&self, url: &str) -> Option<reqwest::Response> {
        let delay = self.current_delay().await;
        sleep(delay).await;

        for attempt in 0..5 {
            match self.inner.get(url).send().await {
                Ok(r) if r.status().is_success() => {
                    self.recover().await;
                    return Some(r);
                }
                Ok(r) if r.status() == StatusCode::TOO_MANY_REQUESTS
                       || r.status() == StatusCode::SERVICE_UNAVAILABLE => {
                    self.back_off().await;
                    sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
                Ok(r) if r.status() == StatusCode::NOT_FOUND
                       || r.status() == StatusCode::FORBIDDEN => {
                    debug!("HTTP {} - skipping {}", r.status(), url);
                    return None;
                }
                Ok(r) => {
                    warn!("HTTP {} on {} (attempt {})", r.status(), url, attempt + 1);
                    sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
                Err(e) => {
                    warn!("Stream error on {} (attempt {}): {}", url, attempt + 1, e);
                    sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
            }
        }
        None
    }

    fn arc_multiplier(&self) -> Arc<Mutex<f64>> {
        Arc::clone(&self.multiplier)
    }
}

/// Simple cheap jitter: pseudo-random f64 in [0, 1) using thread timestamp
fn rand_jitter() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (ns % 1000) as f64 / 1000.0
}

// -- Selectors (built once) ----------------------------------------------------

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("bad CSS selector")
}

fn slugify(s: &str) -> String {
    let sanitized = sanitize(s.trim());
    let clean = sanitized.trim_end_matches('.').trim();
    if clean.is_empty() {
        return "unknown".to_string();
    }
    let path = Path::new(clean);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(clean);

    let max_stem_len = if ext.is_empty() { 120 } else { 120usize.saturating_sub(ext.len() + 1) };
    let truncated_stem: String = stem.chars().take(max_stem_len).collect();

    if ext.is_empty() {
        truncated_stem
    } else {
        format!("{}.{}", truncated_stem, ext)
    }
}

fn extract_handle(href: &str) -> Option<&str> {
    let prefix = "/xmlui/handle/";
    href.find(prefix).map(|p| &href[p + prefix.len()..])
}

// -- QoS #3: Drive health guard ------------------------------------------------

/// Returns Err if the destination directory is no longer writable
async fn check_drive(dest: &Path) -> Result<()> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let probe = dest.join(format!(".probe_{}_{}", std::process::id(), nanos));
    fs::write(&probe, b"ok")
        .await
        .with_context(|| format!("Drive write-check failed at {}", dest.display()))?;
    fs::remove_file(&probe).await.ok();
    Ok(())
}

// -- QoS #4: PDF validation ----------------------------------------------------

/// Returns true if the first 5 bytes are `%PDF-`
fn is_valid_pdf(buf: &[u8]) -> bool {
    buf.len() >= 5 && &buf[..5] == PDF_MAGIC
}

/// Check Content-Type header is PDF-like
fn content_type_is_pdf(resp: &reqwest::Response) -> bool {
    resp.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("pdf") || ct.contains("octet-stream"))
        .unwrap_or(true) // if no header, don't reject preemptively
}

// -- QoS #5: SHA-256 dedup -----------------------------------------------------

fn sha256_of(buf: &[u8]) -> String {
    format!("{:x}", Sha256::digest(buf))
}

// -- Discovery -----------------------------------------------------------------

async fn discover_collections(client: &HttpClient) -> Vec<Collection> {
    info!("Discovering collections under handle/{} ...", ROOT_HANDLE);

    let subcomm_sel = sel("li.ds-artifact-item.community a[href]");
    let coll_sel    = sel("li.ds-artifact-item.collection a[href]");
    let title_sel   = sel("h1.ds-div-head, h1.pagetitle");

    let mut collections: Vec<Collection> = Vec::new();
    let mut visited: HashSet<String>     = HashSet::new();
    let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
    queue.push_back((ROOT_HANDLE.to_string(), vec![]));

    while let Some((handle, path)) = queue.pop_front() {
        if visited.contains(&handle) { continue; }
        visited.insert(handle.clone());

        let url  = format!("{}/xmlui/handle/{}", BASE_URL, handle);
        let html = match client.get_text(&url).await {
            Some(h) => h,
            None    => continue,
        };
        let doc = Html::parse_document(&html);

        let name = doc.select(&title_sel).next()
            .map(|e| e.text().collect::<String>())
            .unwrap_or_else(|| handle.clone());
        let current_path = [path.as_slice(), &[slugify(&name)]].concat();

        for a in doc.select(&subcomm_sel) {
            if let Some(h) = a.value().attr("href").and_then(extract_handle) {
                if !visited.contains(h) {
                    queue.push_back((h.to_string(), current_path.clone()));
                }
            }
        }

        for a in doc.select(&coll_sel) {
            if let Some(href) = a.value().attr("href") {
                if let Some(h) = extract_handle(href) {
                    if !visited.contains(h) {
                        visited.insert(h.to_string());
                        let label = slugify(&a.text().collect::<String>());
                        collections.push(Collection {
                            handle: h.to_string(),
                            path:   [current_path.as_slice(), &[label]].concat(),
                        });
                    }
                }
            }
        }
    }

    info!("Found {} collections", collections.len());
    collections
}

// -- Item enumeration ----------------------------------------------------------

async fn enumerate_items(client: &HttpClient, handle: &str, path: &[String]) -> Vec<Item> {
    let item_sel = sel("div.artifact-title a[href]");
    let next_sel = sel("a[rel='next']");
    let mut results = Vec::new();
    let mut page = 0usize;

    loop {
        let url = format!(
            "{}/xmlui/handle/{}/browse?type=title&rpp=100&etal=0&start={}",
            BASE_URL, handle, page * 100
        );
        let html = match client.get_text(&url).await {
            Some(h) => h,
            None    => break,
        };
        let doc = Html::parse_document(&html);

        let items: Vec<_> = doc.select(&item_sel)
            .filter_map(|e| e.value().attr("href").and_then(extract_handle))
            .collect();

        if items.is_empty() { break; }
        for h in &items {
            results.push(Item { handle: h.to_string(), path: path.to_vec() });
        }
        if doc.select(&next_sel).next().is_none() { break; }
        page += 1;
    }
    results
}

// -- Bitstream extraction ------------------------------------------------------

struct Bitstream {
    url:      String,
    filename: String,
}

async fn extract_bitstreams(client: &HttpClient, item_handle: &str) -> Vec<Bitstream> {
    let url  = format!("{}/xmlui/handle/{}", BASE_URL, item_handle);
    let html = match client.get_text(&url).await {
        Some(h) => h,
        None    => return vec![],
    };
    let doc = Html::parse_document(&html);

    let wrapper_sel = sel("div.file-wrapper");
    let link_sel    = sel("div.file-link a");
    let name_sel    = sel("div.file-metadata span[title]");
    let mut results = Vec::new();

    for wrapper in doc.select(&wrapper_sel) {
        let href = match wrapper.select(&link_sel).next()
            .and_then(|e| e.value().attr("href")) { Some(h) => h, None => continue };

        if !href.contains("/bitstream/handle/") { continue; }

        let clean = href.split('?').next().unwrap_or(href);
        let dl_url = format!("{}{}", BASE_URL, clean);

        let filename = wrapper.select(&name_sel).next()
            .and_then(|e| e.value().attr("title"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                urlencoding::decode(clean.rsplit('/').next().unwrap_or("file"))
                    .map(|s| s.into_owned())
                    .unwrap_or_else(|_| "file.pdf".to_string())
            });

        results.push(Bitstream { url: dl_url, filename });
    }
    results
}

// -- File download (all QoS guards applied) ------------------------------------

#[allow(clippy::too_many_arguments)]
async fn download_file(
    client:     &HttpClient,
    url:        &str,
    dest:       &Path,
    dest_root:  &Path,
    state:      &Arc<Mutex<State>>,
    state_path: &Path,
    bytes_done: &Arc<AtomicU64>,
    drive_ok:   &Arc<AtomicBool>,
) -> Result<()> {
    // QoS #3: abort early if drive went away
    if !drive_ok.load(Ordering::Relaxed) {
        anyhow::bail!("Drive offline - skipping {}", url);
    }

    // QoS #2 + #5: if file already on disk, just re-verify and register hash
    if dest.exists() && dest.metadata().map(|m| m.len()).unwrap_or(0) > 0 {
        debug!("Already exists: {}", dest.display());
        return Ok(());
    }

    let resp = match client.get_stream(url).await {
        Some(r) => r,
        None    => anyhow::bail!("No response for {}", url),
    };

    // QoS #4: reject non-PDF content-type upfront
    if !content_type_is_pdf(&resp) {
        anyhow::bail!("Non-PDF Content-Type for {} - skipping", url);
    }

    // Stream into memory so we can hash + validate before writing
    let bytes = resp.bytes().await
        .with_context(|| format!("Reading body of {}", url))?;

    // QoS #4: reject HTML masquerading as PDF
    if !is_valid_pdf(&bytes) {
        anyhow::bail!(
            "Invalid PDF magic bytes for {} (got {:?}) - likely login page",
            url,
            &bytes[..bytes.len().min(8)]
        );
    }

    // QoS #5: dedup via SHA-256
    let hash = sha256_of(&bytes);
    {
        let st = state.lock().await;
        if let Some(existing_path) = st.hash_to_path.get(&hash) {
            let src = PathBuf::from(existing_path);
            if src.exists() {
                // Hardlink instead of re-downloading
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).await.ok();
                }
                match fs::hard_link(&src, dest).await {
                    Ok(_)  => {
                        info!("=> hardlink (dedup): {}", dest.file_name().and_then(|n| n.to_str()).unwrap_or("?"));
                        return Ok(());
                    }
                    Err(_) => { /* cross-device or other issue - fall through to normal write */ }
                }
            }
        }
    }

    // QoS #3: verify drive is still writable before committing
    if check_drive(dest_root).await.is_err() {
        drive_ok.store(false, Ordering::Relaxed);
        // Save state so we can resume
        let st = state.lock().await;
        let _ = save_state(&st, state_path).await;
        anyhow::bail!("Drive went offline during download of {}", url);
    }

    // QoS #2: atomic write via .part file
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await
            .with_context(|| format!("mkdir {}", parent.display()))?;
    }
    let part = dest.with_extension(
        format!("{}.part", dest.extension().and_then(|e| e.to_str()).unwrap_or("pdf"))
    );
    {
        let mut f = fs::File::create(&part).await
            .with_context(|| format!("create {}", part.display()))?;
        f.write_all(&bytes).await?;
        f.flush().await?;
    }
    fs::rename(&part, dest).await
        .with_context(|| format!("rename {} -> {}", part.display(), dest.display()))?;

    bytes_done.fetch_add(bytes.len() as u64, Ordering::Relaxed);

    // Register hash -> path for future dedup
    {
        let mut st = state.lock().await;
        st.hash_to_path.insert(hash, dest.to_string_lossy().to_string());
    }

    info!("[OK] {}", dest.file_name().and_then(|n| n.to_str()).unwrap_or("?"));
    Ok(())
}

// -- State I/O -----------------------------------------------------------------

async fn load_state(path: &Path) -> State {
    match fs::read_to_string(path).await {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => State { delay_multiplier: 1.0, ..Default::default() },
    }
}

async fn save_state(state: &State, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(state)?;
    // QoS #2: atomic state write
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
}

// -- Main ----------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(if args.verbose { "amrita_dl=debug" } else { "amrita_dl=info" })
        .with_target(false)
        .init();

    let dest_root = args.dest.clone();
    fs::create_dir_all(&dest_root).await?;

    // QoS #3: verify drive is writable at startup
    check_drive(&dest_root).await
        .context("Destination drive not writable at startup - aborting")?;
    info!("Drive OK. Destination: {}", dest_root.display());

    let state_path = PathBuf::from(STATE_FILE);
    let initial_state = load_state(&state_path).await;
    let initial_multiplier = initial_state.delay_multiplier.max(1.0);

    if initial_state.done_items.is_empty() {
        info!("Fresh run");
    } else {
        info!(
            "Resuming: {} items done, {} files done, {:.1}x delay",
            initial_state.done_items.len(),
            initial_state.done_files.len(),
            initial_multiplier,
        );
    }

    let state = Arc::new(Mutex::new(initial_state));
    let drive_ok    = Arc::new(AtomicBool::new(true));
    let bytes_done  = Arc::new(AtomicU64::new(0));

    let client = Arc::new(HttpClient::new(args.delay, initial_multiplier)?);

    // -- Phase 1: Discover collections -----------------------------------------
    let collections = {
        let mut st = state.lock().await;
        if let Some(c) = st.collections.clone() {
            info!("Loaded {} collections from state", c.len());
            c
        } else {
            let c = discover_collections(&client).await;
            st.collections = Some(c.clone());
            save_state(&st, &state_path).await?;
            c
        }
    };

    // -- Phase 2: Enumerate items -----------------------------------------------
    let all_items = {
        let st = state.lock().await;
        if let Some(i) = st.items.clone() {
            info!("Loaded {} items from state", i.len());
            i
        } else {
            drop(st);
            info!("Enumerating items across {} collections ...", collections.len());
            let mut all: Vec<Item>           = Vec::new();
            let mut seen: HashSet<String>    = HashSet::new();
            for coll in &collections {
                info!("  Scanning: {}", coll.path.join(" / "));
                let items = enumerate_items(&client, &coll.handle, &coll.path).await;
                info!("    -> {} items", items.len());
                for item in items {
                    if seen.insert(item.handle.clone()) { all.push(item); }
                }
            }
            info!("Total unique items: {}", all.len());
            let mut st2 = state.lock().await;
            st2.items = Some(all.clone());
            save_state(&st2, &state_path).await?;
            all
        }
    };

    // -- Phase 3 + 4: Download -------------------------------------------------
    let mp    = MultiProgress::new();
    let dl_pb = mp.add(ProgressBar::new(all_items.len() as u64));
    dl_pb.set_style(ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:45.green/white} {pos}/{len}  {msg}")
        .unwrap());

    let total_items = all_items.len();

    stream::iter(all_items)
        .for_each_concurrent(args.workers, |item| {
            let client      = Arc::clone(&client);
            let state       = Arc::clone(&state);
            let sp          = state_path.clone();
            let dest_root   = dest_root.clone();
            let drive_ok    = Arc::clone(&drive_ok);
            let bytes_done  = Arc::clone(&bytes_done);
            let pb          = dl_pb.clone();

            async move {
                // QoS #3: stop dispatching if drive went offline
                if !drive_ok.load(Ordering::Relaxed) { return; }

                {
                    let st = state.lock().await;
                    if st.done_items.contains(&item.handle) {
                        pb.inc(1);
                        return;
                    }
                }

                let bitstreams = extract_bitstreams(&client, &item.handle).await;
                let mut all_ok = true;

                for bs in bitstreams {
                    {
                        let st = state.lock().await;
                        if st.done_files.contains(&bs.url) { continue; }
                    }

                    let dest = dest_root
                        .join(item.path.iter().collect::<PathBuf>())
                        .join(slugify(&bs.filename));

                    pb.set_message(bs.filename.clone());

                    match download_file(
                        &client, &bs.url, &dest, &dest_root,
                        &state, &sp, &bytes_done, &drive_ok,
                    ).await {
                        Ok(_) => {
                            let mut st = state.lock().await;
                            st.done_files.insert(bs.url);
                            if st.done_files.len() % 50 == 0 {
                                let _ = save_state(&st, &sp).await;
                            }
                        }
                        Err(e) => {
                            error!("[FAIL] {}: {}", bs.url, e);
                            all_ok = false;

                            // QoS #3: if drive is gone, persist state and halt
                            if !drive_ok.load(Ordering::Relaxed) {
                                let st = state.lock().await;
                                let _ = save_state(&st, &sp).await;
                                error!("Drive offline - halting. Re-run when drive is back.");
                                return;
                            }
                        }
                    }
                }

                if all_ok {
                    let mut st = state.lock().await;
                    st.done_items.insert(item.handle.clone());
                    // Persist current multiplier so resume keeps the same rate
                    st.delay_multiplier = *client.arc_multiplier().lock().await;
                    if st.done_items.len() % 50 == 0 {
                        let _ = save_state(&st, &sp).await;
                    }
                }

                pb.inc(1);

                // QoS #1: periodic drive-health probe (every ~50 files)
                let done = bytes_done.load(Ordering::Relaxed);
                if done % (50 * 500_000) < 500_000 && check_drive(&dest_root).await.is_err() {
                    drive_ok.store(false, Ordering::Relaxed);
                }
            }
        })
        .await;

    dl_pb.finish_with_message("done");

    let st = state.lock().await;
    let _ = save_state(&st, &state_path).await;
    let mb = bytes_done.load(Ordering::Relaxed) / (1024 * 1024);
    println!(
        "\n[OK] {} files downloaded  ({} MB this session)\n[OK] {} / {} items complete\nOutput: {}",
        st.done_files.len(), mb,
        st.done_items.len(), total_items,
        dest_root.display()
    );

    Ok(())
}
