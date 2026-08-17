//! Amrita Exam Papers Search Server (`amrita-server`)
//!
//! Provides ultra-fast SQLite FTS5 search with BM25 ranking, prefix expansion,
//! synonym mapping, facets pre-loading, data sanitization, and PDF streaming via Axum HTTP.

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
};

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use tokio::fs as tokio_fs;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    indexed_root: PathBuf,
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    department: Option<String>,
    program: Option<String>,
    semester: Option<String>,
    year: Option<String>,
    category: Option<String>,
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

#[derive(Serialize)]
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
        "EndSem" => "End Semester Examination".to_string(),
        "MidTerm" => "Mid Term Assessment".to_string(),
        "Supply" => "Supplementary Examination".to_string(),
        "First Assessment" | "Ass1" | "Ass 1" | "AssI" => "Continuous Assessment I".to_string(),
        "Second Assessment" | "Ass2" | "Ass 2" | "AssII" => "Continuous Assessment II".to_string(),
        "Third Assessment" | "Ass3" | "Ass 3" | "AssIII" => "Continuous Assessment III".to_string(),
        _ => exam.to_string(),
    }
}

fn sanitize_title(title: &str, code: &str) -> String {
    let mut t = title.trim().to_string();
    if t.ends_with(".pdf") || t.ends_with(".PDF") {
        t = t[..t.len() - 4].to_string();
    }
    if let Some(pos) = t.find('_') {
        t = t[..pos].trim().to_string();
    }
    if t.eq_ignore_ascii_case(code) || t.is_empty() {
        t = format!("{code} Examination Paper");
    }
    t
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
    let limit = params.limit.unwrap_or(24).min(500);
    let offset = params.offset.unwrap_or(0);

    let conn = Connection::open_with_flags(
        &state.db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let raw_q = params.q.unwrap_or_default().trim().to_string();
    let has_text_q = !raw_q.is_empty();

    let mut where_clause = String::new();
    let mut bindings: Vec<String> = Vec::new();

    if has_text_q {
        let fts_query = expand_synonyms(&raw_q);
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
            where_clause.push_str(" AND p.year = ? ");
            bindings.push(yr.clone());
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
        sql.push_str(" ORDER BY bm25(papers_fts, 10.0, 5.0, 2.0) ASC LIMIT ? OFFSET ?");
    } else {
        sql.push_str(" ORDER BY p.year DESC, p.course_code ASC LIMIT ? OFFSET ?");
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
            let rel = orig_path
                .strip_prefix(state.indexed_root.to_str().unwrap_or_default())
                .unwrap_or(&orig_path)
                .trim_start_matches('/')
                .to_string();

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
    let conn = Connection::open_with_flags(
        &state.db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut total_papers = 0;
    let mut stmt_total = conn.prepare("SELECT count(*) FROM papers").unwrap();
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
    progs.sort_by(|a, b| b.1.cmp(&a.1));

    // Years restricted dynamically using system clock (1990 to current_year + 1)
    let max_year = get_current_year() + 1;
    let year_sql = format!(
        "SELECT year, count(*) FROM papers WHERE CAST(year AS INTEGER) BETWEEN 1990 AND {} GROUP BY year ORDER BY year DESC",
        max_year
    );
    let yrs = fetch_facet(&year_sql);
    let cats = fetch_facet("SELECT course_category, count(*) FROM papers GROUP BY course_category ORDER BY count(*) DESC");

    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, "public, max-age=300".parse().unwrap());

    Ok((headers, Json(FacetResponse {
        departments: depts,
        programs: progs,
        years: yrs,
        categories: cats,
        total_papers,
    })))
}

async fn handle_suggest(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<SuggestRecord>>, (StatusCode, String)> {
    let q = match params.get("q") {
        Some(v) if !v.trim().is_empty() => v.trim(),
        _ => return Ok(Json(Vec::new())),
    };

    let conn = Connection::open_with_flags(
        &state.db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pattern = format!("{q}%");
    let mut stmt = conn
        .prepare("SELECT DISTINCT course_code, course_title, department, program, original_path FROM papers WHERE course_code LIKE ? OR course_title LIKE ? ORDER BY course_code ASC LIMIT 8")
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
    for r in rows.flatten() {
        suggestions.push(r);
    }

    Ok(Json(suggestions))
}

async fn handle_pdf(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let rel_path = match params.get("path") {
        Some(p) => p,
        None => return (StatusCode::BAD_REQUEST, "Missing path parameter").into_response(),
    };

    // Sanitize path against directory traversal attacks
    if rel_path.contains("..") || rel_path.contains("\\..") {
        return (StatusCode::FORBIDDEN, "Access Denied: Path Traversal Detected").into_response();
    }

    let path_str = if rel_path.starts_with('/') {
        rel_path.to_string()
    } else {
        format!("/{rel_path}")
    };

    let full_path = if std::path::Path::new(&path_str).exists() {
        PathBuf::from(&path_str)
    } else if std::path::Path::new(rel_path).exists() {
        PathBuf::from(rel_path)
    } else {
        state.indexed_root.join(rel_path.trim_start_matches('/'))
    };

    let canonical = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, "PDF File Not Found").into_response(),
    };

    let root_canonical = state.indexed_root.canonicalize().unwrap_or_else(|_| state.indexed_root.clone());
    if !canonical.starts_with(&root_canonical) {
        return (StatusCode::FORBIDDEN, "Access Denied: Path outside indexed root").into_response();
    }

    if canonical.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some("pdf".to_string()) {
        return (StatusCode::FORBIDDEN, "Access Denied: Only PDF files can be served").into_response();
    }

    match tokio_fs::read(&canonical).await {
        Ok(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, "application/pdf".parse().unwrap());
            headers.insert(header::CONTENT_DISPOSITION, "inline".parse().unwrap());
            (headers, bytes).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read PDF").into_response(),
    }
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

    let indexed_root = PathBuf::from("/run/media/anuruprkris/DATA/amrita-exam-papers-indexed");
    let db_path = indexed_root.join("index.db");

    if !db_path.exists() {
        eprintln!("Error: index.db not found at {}", db_path.display());
        std::process::exit(1);
    }

    let shared_state = Arc::new(AppState {
        db_path,
        indexed_root,
    });

    let app = Router::new()
        .route("/", get(handle_index))
        .route("/api/search", get(handle_search))
        .route("/api/facets", get(handle_facets))
        .route("/api/suggest", get(handle_suggest))
        .route("/api/pdf", get(handle_pdf))
        .nest_service("/static", ServeDir::new("web"))
        .layer(CorsLayer::permissive())
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("🚀 Amrita Exam Papers Search Server listening on http://localhost:8080");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
