//! Amrita Exam Papers Search Server (`amrita-server`)
//!
//! Provides ultra-fast SQLite FTS5 search with BM25 ranking, prefix expansion,
//! synonym mapping, facets pre-loading, data sanitization, and PDF streaming via Axum HTTP.

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, LazyLock, RwLock},
    time::{Duration, Instant},
};

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OpenFlags};
use serde::{Deserialize, Serialize};
use tokio::fs as tokio_fs;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

static RE_NUM_LETTER: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(\d)([A-Za-z])").unwrap()
});
static RE_ROMAN_CONCAT: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b([a-z]{3,})(I{2,3}|IV|VI{0,3}|IX|XI{0,2})\b").unwrap()
});
static RE_CODE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(\b\d{2})?[A-Z]{2,6}\d{3,4}[A-Z]?\b").unwrap()
});
static RE_SPACES: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\s+").unwrap()
});

#[derive(Clone)]
struct CachedFacets {
    response: FacetResponse,
    computed_at: Instant,
}

#[derive(Clone)]
struct AppState {
    db_pool: Pool<SqliteConnectionManager>,
    indexed_root: PathBuf,
    raw_root: PathBuf,
    storage_public_url: Option<String>,
    facet_cache: Arc<RwLock<Option<CachedFacets>>>,
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    department: Option<String>,
    program: Option<String>,
    semester: Option<String>,
    year: Option<String>,
    year_end: Option<String>,
    category: Option<String>,
    sort: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<PaperRecord>,
    total: usize,
    offset: usize,
    limit: usize,
}

#[derive(Serialize)]
struct PaperRecord {
    id: i64,
    course_code: String,
    course_title: String,
    department: String,
    program: String,
    semester: String,
    year: String,
    exam_type: String,
    course_category: String,
    course_level: String,
    relative_path: String,
}

#[derive(Serialize, Clone)]
struct FacetResponse {
    departments: Vec<(String, usize)>,
    programs: Vec<(String, usize)>,
    years: Vec<(String, usize)>,
    categories: Vec<(String, usize)>,
    total_papers: usize,
}

#[derive(Serialize)]
struct SuggestRecord {
    course_code: String,
    course_title: String,
    department: String,
    program: String,
}

#[derive(Deserialize)]
struct BatchDownloadRequest {
    paths: Vec<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    database: String,
    total_papers: usize,
}

fn get_current_year() -> i32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    (1970 + (secs / 31556926)) as i32
}

fn sanitize_year(year: &str) -> String {
    let y: i32 = year.parse().unwrap_or(0);
    let max_year = get_current_year() + 1;
    if y >= 1990 && y <= max_year {
        y.to_string()
    } else {
        get_current_year().to_string()
    }
}

fn sanitize_program(prog: &str, dept: &str, orig_path: &str) -> String {
    let p = prog.trim();
    if p.eq_ignore_ascii_case("BTech") || p.eq_ignore_ascii_case("B.Tech") {
        "B.Tech".to_string()
    } else if p.eq_ignore_ascii_case("MTech") || p.eq_ignore_ascii_case("M.Tech") {
        "M.Tech".to_string()
    } else if p == "General" || p == "Sciences" || p.is_empty() {
        if orig_path.contains("B.Tech") || dept.contains("Engineering") || dept.contains("Computer") || dept.contains("Electronics") {
            "B.Tech".to_string()
        } else if orig_path.contains("M.Tech") {
            "M.Tech".to_string()
        } else if orig_path.contains("MCA") {
            "MCA".to_string()
        } else if dept.contains("Humanities") || dept.contains("Media") {
            "B.A.".to_string()
        } else if dept.contains("Mathematics") || dept.contains("Physical") || dept.contains("Chemical") {
            "M.Sc.".to_string()
        } else {
            "B.Tech".to_string()
        }
    } else if p.eq_ignore_ascii_case("Communicaiton") || p.contains("Communication") {
        "B.A. Communication".to_string()
    } else if p.eq_ignore_ascii_case("IntMSc") || p.eq_ignore_ascii_case("Integrated MSc") {
        "Integrated M.Sc.".to_string()
    } else {
        p.to_string()
    }
}

fn format_semester(sem: &str) -> String {
    let s = sem.trim();
    match s {
        "Sem01" | "1st Semester" => "Semester I".to_string(),
        "Sem02" | "2nd Semester" => "Semester II".to_string(),
        "Sem03" | "3rd Semester" => "Semester III".to_string(),
        "Sem04" | "4th Semester" => "Semester IV".to_string(),
        "Sem05" | "5th Semester" => "Semester V".to_string(),
        "Sem06" | "6th Semester" => "Semester VI".to_string(),
        "Sem07" | "7th Semester" => "Semester VII".to_string(),
        "Sem08" | "8th Semester" => "Semester VIII".to_string(),
        _ if s.starts_with("Sem") => format!("Semester {}", s.trim_start_matches("Sem")),
        _ => s.to_string(),
    }
}

fn format_exam_type(exam: &str) -> String {
    match exam.trim() {
        "EndSem" => "End Sem".to_string(),
        "MidTerm" => "Mid Sem".to_string(),
        "Supply" => "Supply".to_string(),
        "First Assessment" | "Ass1" | "Ass 1" | "AssI" => "CA I".to_string(),
        "Second Assessment" | "Ass2" | "Ass 2" | "AssII" => "CA II".to_string(),
        "Third Assessment" | "Ass3" | "Ass 3" | "AssIII" => "CA III".to_string(),
        _ => exam.to_string(),
    }
}

fn sanitize_title(title: &str, code: &str) -> String {
    let mut t = title.trim().to_string();

    // Strip trailing .pdf extension
    if t.ends_with(".pdf") || t.ends_with(".PDF") {
        t = t[..t.len() - 4].trim().to_string();
    }

    // Strip everything from first underscore (filename metadata starts there)
    if let Some(pos) = t.find('_') {
        t = t[..pos].trim().to_string();
    }

    // Insert space between digits and following letters (e.g. "21vl601Embedded" -> "21vl601 Embedded")
    t = RE_NUM_LETTER.replace_all(&t, "$1 $2").to_string();

    // Insert space before Roman numerals concatenated to words: e.g. "MalayalamII" -> "Malayalam II"
    t = RE_ROMAN_CONCAT.replace_all(&t, "$1 $2").to_string();

    // Strip ALL leading non-alphabetic characters (handles &, /, ,, ., -, spaces, etc.)
    t = t.trim_start_matches(|c: char| !c.is_alphabetic()).to_string();

    // Remove course code patterns:
    // 1) Standard codes with or without 2-digit year prefix: 21TAM101, 24AI632, 21VL601, OL832, CHY251, RM610, CS602
    t = RE_CODE.replace_all(&t, " ").trim().to_string();

    // Clean up remaining punctuation: replace (, ), [, ], /, -, :, comma with space
    t = t.chars().map(|c| match c {
        '(' | ')' | '[' | ']' | '/' | '-' | ':' | ',' | '.' => ' ',
        _ => c,
    }).collect();

    // Strip any remaining leading non-alpha after code & punctuation removal
    t = t.trim_start_matches(|c: char| !c.is_alphabetic()).to_string();

    // Collapse multiple spaces
    t = RE_SPACES.replace_all(&t, " ").trim().to_string();

    // Detect garbage titles (PDF question text leaked through extraction or exam metadata stored as title)
    let garbage_patterns = [
        "mod 19", "LFSR", "shifted from", "and the second", "Reflections", 
        "Nucleons", "Binding Energy", "0 V when", "802.11", "Researchers",
        "CO-1", "CO-2", "BL-1", "BL-2", "CO 1", "BL 1",
        "semester", "assessment", "mid term", "mid-term", "end term", "end-term",
        "regular", "supplymentary", "supplementary", "first sem", "second sem",
        "third sem", "fourth sem", "fifth sem", "sixth sem", "seventh sem", "eighth sem",
    ];
    let lower_t = t.to_lowercase();
    let is_garbage = garbage_patterns.iter().any(|p| lower_t.contains(&p.to_lowercase()))
        || lower_t.starts_with("of ")
        || lower_t.starts_with("with ")
        || lower_t.starts_with("and ")
        || lower_t.starts_with("the ")
        || lower_t.starts_with("are ")
        || lower_t.starts_with("is ")
        || lower_t.starts_with("in ")
        || lower_t.starts_with("ieee ")
        || t.starts_with('0')
        || t.starts_with('1')
        || t.starts_with('2')
        || t.starts_with('3')
        || t.starts_with('4')
        || t.starts_with('5')
        || t.starts_with('6')
        || t.starts_with('7')
        || t.starts_with('8')
        || t.starts_with('9')
        || t.len() < 3
        || t.eq_ignore_ascii_case(code);

    if is_garbage {
        let clean_code = sanitize_code(code);
        return format!("{clean_code} Examination Paper");
    }

    // Title Case normalization with Roman numeral and small-words preservation
    let small_words = ["and", "or", "of", "the", "in", "for", "a", "an", "to", "on", "at", "by", "with", "from", "its"];
    let roman_numerals = ["II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII"];

    let mut words: Vec<String> = t.split_whitespace().enumerate().map(|(i, w)| {
        let lower = w.to_lowercase();

        // Check if it's a Roman numeral (I, II, III, IV, V, etc.) - keep uppercase
        if roman_numerals.iter().any(|r| r.eq_ignore_ascii_case(w)) {
            return w.to_uppercase();
        }
        // Single I is Roman if position > 0 or multiple words
        if w.eq_ignore_ascii_case("I") && (i > 0 || t.split_whitespace().count() > 1) {
            return "I".to_string();
        }

        // Small words (AND, OR, OF, etc.) should always be lowercase unless it's the very first word
        if i > 0 && small_words.contains(&lower.as_str()) {
            return lower;
        }

        // Preserve all-caps acronyms (2-4 letters, not a small word)
        if w.len() >= 2 && w.len() <= 4
            && w.chars().all(|c| c.is_uppercase())
            && w.chars().any(|c| c.is_alphabetic())
            && !small_words.contains(&lower.as_str())
        {
            return w.to_string();
        }

        // Normal Title Case: capitalize first letter
        let mut chars = lower.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    }).collect();

    // Dedupe repeated adjacent word phrases of any length (1-word, 2-word, 3-word, etc.)
    // e.g. "Research Methodology Research Methodology" -> "Research Methodology"
    let mut i = 0;
    while i < words.len() {
        let mut found_dup = false;
        let max_k = (words.len() - i) / 2;
        for k in (1..=max_k).rev() {
            let slice1 = &words[i..i+k];
            let slice2 = &words[i+k..i+2*k];
            let is_match = slice1.iter().zip(slice2.iter()).all(|(a, b)| a.eq_ignore_ascii_case(b));
            if is_match {
                words.drain(i..i+k);
                found_dup = true;
                break;
            }
        }
        if !found_dup {
            i += 1;
        }
    }

    let result = words.join(" ");
    if result.is_empty() || result.len() < 3 {
        let clean_code = sanitize_code(code);
        format!("{clean_code} Examination Paper")
    } else {
        result
    }
}

fn sanitize_code(code: &str) -> String {
    // Clean up garbage course codes
    let code = code.trim();
    if code.contains(' ') 
        || code.contains(',') 
        || code.len() < 3 
        || code.chars().all(|c| c.is_ascii_digit())
        || code.to_lowercase().starts_with("the ")
        || code.to_lowercase().starts_with("with ")
        || code.to_lowercase().starts_with("and ")
        || code.to_lowercase().starts_with("of ")
        || code.to_lowercase().starts_with("in ")
        || code.to_lowercase().starts_with("are ")
        || code.to_lowercase().starts_with("is ")
    {
        "AMRITA".to_string()
    } else {
        code.to_uppercase()
    }
}

fn format_level(level: &str) -> String {
    if level.contains("700") || level.contains("800") || level.contains("PhD") {
        "Doctoral & Research".to_string()
    } else if level.contains("500") || level.contains("600") {
        "Postgraduate Core".to_string()
    } else if level.contains("300") || level.contains("400") {
        "Undergraduate Major".to_string()
    } else {
        "Undergraduate Core".to_string()
    }
}

fn sanitize_fts_query(query: &str) -> String {
    // Remove FTS5 special characters that cause syntax errors
    let cleaned: String = query
        .chars()
        .map(|c| match c {
            '"' | '*' | ':' | '(' | ')' | '\n' | '\r' => ' ',
            _ => c,
        })
        .collect();

    // Remove FTS5 keywords
    let keywords = ["AND", "OR", "NOT", "NEAR"];
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let filtered: Vec<&str> = words
        .iter()
        .filter(|w| !keywords.contains(&w.to_uppercase().as_str()))
        .copied()
        .collect();
    filtered.join(" ")
}

fn expand_synonyms(query: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();

    for token in query.split_whitespace() {
        let upper = token.to_uppercase();
        let expanded = match upper.as_str() {
            "OS" => "(OS OR \"Operating Systems\")",
            "AI" => "(AI OR \"Artificial Intelligence\")",
            "ML" => "(ML OR \"Machine Learning\")",
            "DS" | "DSA" => "(DSA OR \"Data Structures\")",
            "CN" => "(CN OR \"Computer Networks\")",
            "DBMS" => "(DBMS OR \"Database Management\")",
            "CV" => "(CV OR \"Computer Vision\")",
            "CVL" => "(CVL OR Civil)",
            "MAT" | "MATH" => "(MAT OR Mathematics)",
            _ => {
                if token.len() >= 2 && !token.ends_with('*') && !token.contains('"') {
                    &format!("{token}*")
                } else {
                    token
                }
            }
        };
        tokens.push(expanded.to_string());
    }

    tokens.join(" ")
}

async fn handle_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(24).clamp(1, 100);
    let offset = params.offset.unwrap_or(0);

    let conn = state.db_pool.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let raw_q = params.q.unwrap_or_default().trim().to_string();
    let sanitized = if !raw_q.is_empty() { sanitize_fts_query(&raw_q) } else { String::new() };
    let fts_query = if !sanitized.is_empty() { expand_synonyms(&sanitized) } else { String::new() };
    let has_text_q = !fts_query.trim().is_empty();

    let mut where_clause = String::new();
    let mut bindings: Vec<String> = Vec::new();

    if has_text_q {
        where_clause.push_str(" JOIN papers_fts fts ON p.id = fts.rowid WHERE papers_fts MATCH ? ");
        bindings.push(fts_query);
    } else {
        where_clause.push_str(" WHERE 1=1 ");
    }

    if let Some(ref dept) = params.department {
        if !dept.is_empty() && dept != "All" {
            where_clause.push_str(" AND p.department = ? ");
            bindings.push(dept.clone());
        }
    }

    if let Some(ref prog) = params.program {
        if !prog.is_empty() && prog != "All" {
            where_clause.push_str(" AND p.program = ? ");
            bindings.push(prog.clone());
        }
    }

    if let Some(ref sem) = params.semester {
        if !sem.is_empty() && sem != "All" {
            where_clause.push_str(" AND p.semester = ? ");
            bindings.push(sem.clone());
        }
    }

    if let Some(ref yr) = params.year {
        if !yr.is_empty() && yr != "All" {
            if let Some(ref yr_end) = params.year_end {
                if !yr_end.is_empty() && yr_end != "All" {
                    where_clause.push_str(" AND CAST(p.year AS INTEGER) BETWEEN ? AND ? ");
                    bindings.push(yr.clone());
                    bindings.push(yr_end.clone());
                } else {
                    where_clause.push_str(" AND p.year = ? ");
                    bindings.push(yr.clone());
                }
            } else {
                where_clause.push_str(" AND p.year = ? ");
                bindings.push(yr.clone());
            }
        }
    }

    if let Some(ref cat) = params.category {
        if !cat.is_empty() && cat != "All" {
            where_clause.push_str(" AND p.course_category = ? ");
            bindings.push(cat.clone());
        }
    }

    // 1. Compute total count for pagination
    let count_sql = format!("SELECT count(*) FROM papers p {where_clause}");
    let mut count_stmt = conn.prepare(&count_sql).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rusqlite_count_params: Vec<&dyn rusqlite::ToSql> = bindings.iter().map(|b| b as &dyn rusqlite::ToSql).collect();
    let total: usize = count_stmt.query_row(rusqlite_count_params.as_slice(), |r| r.get(0)).unwrap_or(0);

    // 2. Fetch paginated result records
    let mut sql = format!("SELECT p.id, p.course_code, p.course_title, p.department, p.program, p.semester, p.year, p.exam_type, p.course_category, p.course_level, p.original_path FROM papers p {where_clause}");

    if has_text_q {
        sql.push_str(" ORDER BY bm25(papers_fts, 10.0, 5.0, 2.0, 1.0, 1.0, 1.0, 1.0) ASC LIMIT ? OFFSET ?");
    } else {
        match params.sort.as_deref() {
            Some("code_asc") => sql.push_str(" ORDER BY p.course_code ASC LIMIT ? OFFSET ?"),
            Some("year_asc") => sql.push_str(" ORDER BY p.year ASC, p.course_code ASC LIMIT ? OFFSET ?"),
            Some("title_asc") => sql.push_str(" ORDER BY p.course_title ASC LIMIT ? OFFSET ?"),
            _ => sql.push_str(" ORDER BY p.year DESC, p.course_code ASC LIMIT ? OFFSET ?"),
        }
    }

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rusqlite_params: Vec<&dyn rusqlite::ToSql> = bindings.iter().map(|b| b as &dyn rusqlite::ToSql).collect();
    rusqlite_params.push(&limit);
    rusqlite_params.push(&offset);

    let rows = stmt
        .query_map(rusqlite_params.as_slice(), |row| {
            let orig_path: String = row.get(10)?;
            let orig_p = std::path::Path::new(&orig_path);
            let rel = if let Ok(stripped) = orig_p.strip_prefix(&state.raw_root) {
                stripped.to_string_lossy().to_string()
            } else if let Ok(stripped) = orig_p.strip_prefix(&state.indexed_root) {
                stripped.to_string_lossy().to_string()
            } else if let Some(pos) = orig_path.find("Examination  Papers/") {
                orig_path[pos..].to_string()
            } else if let Some(pos) = orig_path.find("Examination Papers/") {
                orig_path[pos..].to_string()
            } else {
                orig_path.trim_start_matches('/').to_string()
            };

            let code: String = row.get(1)?;
            let raw_title: String = row.get(2)?;
            let dept: String = row.get(3)?;
            let raw_prog: String = row.get(4)?;
            let raw_sem: String = row.get(5)?;
            let raw_year: String = row.get(6)?;
            let raw_exam: String = row.get(7)?;
            let raw_level: String = row.get(9)?;

            Ok(PaperRecord {
                id: row.get(0)?,
                course_code: if code.is_empty() { "AMRITA".to_string() } else { code.clone() },
                course_title: sanitize_title(&raw_title, &code),
                department: dept.clone(),
                program: sanitize_program(&raw_prog, &dept, &orig_path),
                semester: format_semester(&raw_sem),
                year: sanitize_year(&raw_year),
                exam_type: format_exam_type(&raw_exam),
                course_category: row.get(8)?,
                course_level: format_level(&raw_level),
                relative_path: rel,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut records = Vec::new();
    for r in rows.flatten() {
        records.push(r);
    }

    Ok(Json(SearchResponse {
        results: records,
        total,
        offset,
        limit,
    }))
}

async fn handle_facets(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Check in-memory facet cache first with TTL validation (300s)
    if let Ok(cache) = state.facet_cache.read() {
        if let Some(cached) = cache.as_ref() {
            if cached.computed_at.elapsed() < Duration::from_secs(300) {
                let mut headers = HeaderMap::new();
                headers.insert(header::CACHE_CONTROL, "public, max-age=300".parse().unwrap());
                return Ok((headers, Json(cached.response.clone())));
            }
        }
    }

    let conn = state.db_pool.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut total_papers = 0;
    let mut stmt_total = conn.prepare("SELECT count(*) FROM papers")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Ok(mut rows) = stmt_total.query([]) {
        if let Ok(Some(row)) = rows.next() {
            total_papers = row.get(0).unwrap_or(0);
        }
    }

    let fetch_facet = |sql: &str| -> Vec<(String, usize)> {
        let mut list = Vec::new();
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))) {
                for r in rows.flatten() {
                    list.push(r);
                }
            }
        }
        list
    };

    let depts = fetch_facet("SELECT department, count(*) FROM papers GROUP BY department ORDER BY count(*) DESC");
    
    // Program facets with General & Sciences remapped to clean degree titles
    let raw_progs = fetch_facet("SELECT program, count(*) FROM papers GROUP BY program ORDER BY count(*) DESC");
    let mut progs_map: HashMap<String, usize> = HashMap::new();
    for (p, count) in raw_progs {
        let clean_p = match p.as_str() {
            "General" | "BTech" => "B.Tech".to_string(),
            "MTech" => "M.Tech".to_string(),
            "Sciences" => "M.Sc.".to_string(),
            "Communicaiton" => "B.A. Communication".to_string(),
            "IntMSc" => "Integrated M.Sc.".to_string(),
            _ => p,
        };
        *progs_map.entry(clean_p).or_insert(0) += count;
    }
    let mut progs: Vec<(String, usize)> = progs_map.into_iter().collect();
    progs.sort_by_key(|b| std::cmp::Reverse(b.1));

    // Years restricted dynamically using system clock (1990 to current_year + 1)
    let max_year = get_current_year() + 1;
    let year_sql = format!(
        "SELECT year, count(*) FROM papers WHERE CAST(year AS INTEGER) BETWEEN 1990 AND {} GROUP BY year ORDER BY year DESC",
        max_year
    );
    let yrs = fetch_facet(&year_sql);
    let cats = fetch_facet("SELECT course_category, count(*) FROM papers GROUP BY course_category ORDER BY count(*) DESC");

    let facet_response = FacetResponse {
        departments: depts,
        programs: progs,
        years: yrs,
        categories: cats,
        total_papers,
    };

    // Store in cache with current timestamp for subsequent requests
    if let Ok(mut cache) = state.facet_cache.write() {
        *cache = Some(CachedFacets {
            response: facet_response.clone(),
            computed_at: Instant::now(),
        });
    }

    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, "public, max-age=300".parse().unwrap());

    Ok((headers, Json(facet_response)))
}

async fn handle_suggest(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<SuggestRecord>>, (StatusCode, String)> {
    let q = match params.get("q") {
        Some(v) if !v.trim().is_empty() => v.trim(),
        _ => return Ok(Json(Vec::new())),
    };

    let conn = state.db_pool.get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pattern = format!("{q}%");
    let mut stmt = conn
        .prepare("SELECT DISTINCT course_code, course_title, department, program, original_path FROM papers WHERE course_code LIKE ? OR course_title LIKE ? ORDER BY course_code ASC LIMIT 30")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rows = stmt
        .query_map(params![pattern, pattern], |row| {
            let code: String = row.get(0)?;
            let raw_title: String = row.get(1)?;
            let dept: String = row.get(2)?;
            let raw_prog: String = row.get(3)?;
            let orig_path: String = row.get(4)?;

            Ok(SuggestRecord {
                course_code: code.clone(),
                course_title: sanitize_title(&raw_title, &code),
                department: dept.clone(),
                program: sanitize_program(&raw_prog, &dept, &orig_path),
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut suggestions = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for r in rows.flatten() {
        let key = (r.course_code.clone(), r.course_title.clone());
        if seen.insert(key) {
            suggestions.push(r);
            if suggestions.len() >= 8 {
                break;
            }
        }
    }

    Ok(Json(suggestions))
}

fn resolve_pdf_path(identifier: &str, state: &AppState) -> Option<PathBuf> {
    if identifier.contains("..") || identifier.contains("\\..") {
        return None;
    }

    let decoded = urlencoding::decode(identifier).unwrap_or(std::borrow::Cow::Borrowed(identifier));
    let clean = decoded.trim().trim_start_matches('/');

    let conn = state.db_pool.get().ok()?;
    let validated_path: String = if let Ok(id) = clean.parse::<i64>() {
        conn.query_row(
            "SELECT relative_path FROM papers WHERE id = ?1",
            params![id],
            |r| r.get(0),
        ).ok()?
    } else {
        conn.query_row(
            "SELECT relative_path FROM papers WHERE relative_path = ?1 OR original_path = ?1 LIMIT 1",
            params![clean],
            |r| r.get(0),
        ).ok()?
    };

    // If Storage CDN redirect mode is active, return DB-validated path without disk existence check
    if state.storage_public_url.is_some() {
        return Some(state.indexed_root.join(&validated_path));
    }

    let raw_canonical = state.raw_root.canonicalize().ok();
    let indexed_canonical = state.indexed_root.canonicalize().ok();

    let candidates = [
        state.indexed_root.join(&validated_path),
        state.raw_root.join(&validated_path),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            if let Ok(canonical) = candidate.canonicalize() {
                let is_allowed = match (&raw_canonical, &indexed_canonical) {
                    (Some(raw), Some(idx)) => canonical.starts_with(raw) || canonical.starts_with(idx),
                    (Some(raw), None) => canonical.starts_with(raw),
                    (None, Some(idx)) => canonical.starts_with(idx),
                    (None, None) => true,
                };

                if is_allowed && canonical.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) == Some("pdf".to_string()) {
                    return Some(canonical);
                }
            }
        }
    }

    None
}

async fn handle_pdf(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let id_or_path = if let Some(id_str) = params.get("id") {
        id_str.clone()
    } else if let Some(p) = params.get("path") {
        if p.contains("..") || p.contains("\\..") {
            return (StatusCode::BAD_REQUEST, "Invalid path parameter").into_response();
        }
        p.clone()
    } else {
        return (StatusCode::BAD_REQUEST, "Missing id or path parameter").into_response();
    };

    let canonical = match resolve_pdf_path(&id_or_path, &state) {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "PDF File Not Found").into_response(),
    };

    if let Some(ref storage_base) = state.storage_public_url {
        let clean_storage_base = storage_base.trim_end_matches('/');
        let rel_path = canonical
            .strip_prefix(&state.indexed_root)
            .or_else(|_| canonical.strip_prefix(&state.raw_root))
            .unwrap_or(&canonical);

        let encoded_segments: Vec<String> = rel_path
            .components()
            .map(|c| urlencoding::encode(&c.as_os_str().to_string_lossy()).to_string())
            .collect();
        let encoded_rel_path = encoded_segments.join("/");

        let redirect_url = format!("{clean_storage_base}/{encoded_rel_path}");
        if let Ok(header_val) = header::HeaderValue::from_str(&redirect_url) {
            let mut headers = HeaderMap::new();
            headers.insert(header::LOCATION, header_val);
            headers.insert(header::CACHE_CONTROL, "public, max-age=86400".parse().unwrap());
            return (StatusCode::FOUND, headers, ()).into_response();
        }
    }

    match tokio_fs::read(&canonical).await {
        Ok(bytes) => {
            let filename = canonical
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("document.pdf");

            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, "application/pdf".parse().unwrap());
            headers.insert(
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{}\"", filename).parse().unwrap(),
            );
            headers.insert(
                header::CACHE_CONTROL,
                "public, max-age=86400, immutable".parse().unwrap(),
            );
            (headers, bytes).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read PDF").into_response(),
    }
}

async fn handle_health(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let conn = match state.db_pool.get() {
        Ok(c) => c,
        Err(_) => return Json(HealthResponse {
            status: "degraded".to_string(),
            database: "connection_failed".to_string(),
            total_papers: 0,
        }),
    };

    let total: usize = conn
        .query_row("SELECT count(*) FROM papers", [], |r| r.get(0))
        .unwrap_or(0);

    Json(HealthResponse {
        status: "ok".to_string(),
        database: "connected".to_string(),
        total_papers: total,
    })
}

async fn handle_batch_download(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchDownloadRequest>,
) -> impl IntoResponse {
    if req.paths.is_empty() || req.paths.len() > 50 {
        return (StatusCode::BAD_REQUEST, "Invalid number of paths (1-50 allowed)").into_response();
    }

    let mut zip_buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
        let zip_opts = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        for rel_path in &req.paths {
            if let Some(canonical) = resolve_pdf_path(rel_path, &state) {
                let filename = canonical
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("document.pdf");

                if let Ok(bytes) = tokio_fs::read(&canonical).await {
                    if zip.start_file(filename, zip_opts).is_ok() {
                        use std::io::Write;
                        let _ = zip.write_all(&bytes);
                    }
                }
            }
        }
        let _ = zip.finish();
    }

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/zip".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"amrita_exam_papers.zip\"".parse().unwrap(),
    );

    (headers, zip_buf).into_response()
}

async fn handle_index() -> Html<&'static str> {
    Html(include_str!("../../web/index.html"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let raw_root = PathBuf::from(std::env::var("RAW_ROOT").unwrap_or_else(|_| "./amrita-exam-papers".to_string()));
    let indexed_root = PathBuf::from(std::env::var("INDEXED_ROOT").unwrap_or_else(|_| "./amrita-exam-papers-indexed".to_string()));
    let db_path = std::env::var("INDEX_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if PathBuf::from("./index.db").exists() {
                PathBuf::from("./index.db")
            } else {
                indexed_root.join("index.db")
            }
        });

    if !db_path.exists() {
        eprintln!("Error: index.db not found at {}", db_path.display());
        std::process::exit(1);
    }

    let manager = SqliteConnectionManager::file(&db_path)
        .with_flags(OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .with_init(|c| {
            c.execute_batch("PRAGMA query_only = ON; PRAGMA cache_size = -64000; PRAGMA temp_store = MEMORY; PRAGMA mmap_size = 268435456;")?;
            Ok(())
        });
    let db_pool = Pool::builder()
        .max_size(8)
        .build(manager)
        .map_err(|e| {
            eprintln!("Error: failed to create DB pool: {}", e);
            e
        })?;

    let storage_public_url = std::env::var("STORAGE_PUBLIC_URL")
        .or_else(|_| std::env::var("OCI_PUBLIC_URL"))
        .ok();
    if let Some(ref url) = storage_public_url {
        info!("Storage CDN redirect enabled: {url}");
    }

    let shared_state = Arc::new(AppState {
        db_pool,
        indexed_root,
        raw_root,
        storage_public_url,
        facet_cache: Arc::new(RwLock::new(None)),
    });

    let app = Router::new()
        .route("/", get(handle_index))
        .route("/api/search", get(handle_search))
        .route("/api/facets", get(handle_facets))
        .route("/api/suggest", get(handle_suggest))
        .route("/api/pdf", get(handle_pdf))
        .route("/api/health", get(handle_health))
        .route("/api/download-batch", post(handle_batch_download))
        .nest_service("/static", ServeDir::new("web"))
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            header::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            header::HeaderValue::from_static("SAMEORIGIN"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            header::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .with_state(shared_state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Amrita Exam Papers Search Server listening on http://0.0.0.0:{port}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            info!("Received shutdown signal, terminating server gracefully");
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_electrochemistry() {
        assert_eq!(sanitize_title("& Electrochemistry II", "15CHY203"), "Electrochemistry II");
    }

    #[test]
    fn test_research_methodology_dedupe() {
        assert_eq!(sanitize_title("/ 21RM616 Research Methodology/Research Methodology", "21RM616"), "Research Methodology");
    }

    #[test]
    fn test_lowercase_code() {
        assert_eq!(sanitize_title("& OL832 Material Characterization", "OL832"), "Material Characterization");
    }

    #[test]
    fn test_malayalam_concat() {
        assert_eq!(sanitize_title(", 21MAL111, MalayalamII", "21MAL111"), "Malayalam II");
    }

    #[test]
    fn test_algebra_parenthesis() {
        assert_eq!(sanitize_title("Algebra Iii(linear Algebra", "23DLS501"), "Algebra III Linear Algebra");
    }

    #[test]
    fn test_allcaps_and() {
        assert_eq!(sanitize_title(", COMPOSITE MATERIALS AND PROCESSING", "16ME707"), "Composite Materials and Processing");
    }

    #[test]
    fn test_embedded_code_prefix() {
        assert_eq!(sanitize_title("/ 21vl601Embedded Computing and Programming", "21VL601"), "Embedded Computing and Programming");
    }

    #[test]
    fn test_adavanced_typo_code() {
        assert_eq!(sanitize_title("/ 24AI632Adavanced Data Structure", "24AI632"), "Adavanced Data Structure");
    }

    #[test]
    fn test_computational_linear_algebra() {
        assert_eq!(sanitize_title(". Computational Linear Algebra and its Applications", "MA602"), "Computational Linear Algebra and its Applications");
    }

    #[test]
    fn test_reflections_garbage() {
        assert_eq!(sanitize_title(", 002 and 111 reflections.", "15PHY515"), "15PHY515 Examination Paper");
    }

    #[test]
    fn test_semester_metadata_garbage() {
        assert_eq!(sanitize_title("12ELL202 (Third Semester 2015 Regular", "12ELL202"), "12ELL202 Examination Paper");
    }
}
