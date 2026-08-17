//! Amrita Exam Papers Search Server (`amrita-server`)
//!
//! Provides ultra-fast SQLite FTS5 search with BM25 ranking, prefix expansion,
//! synonym mapping, facets pre-loading, and PDF file streaming via Axum HTTP.

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
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use tokio::fs as tokio_fs;
use tower_http::{cors::CorsLayer, services::{ServeDir, ServeFile}};
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
) -> Result<Json<Vec<PaperRecord>>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(50).min(500);

    let conn = Connection::open_with_flags(
        &state.db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let raw_q = params.q.unwrap_or_default().trim().to_string();
    let has_text_q = !raw_q.is_empty();

    let mut sql = String::new();
    let mut bindings: Vec<String> = Vec::new();

    if has_text_q {
        let fts_query = expand_synonyms(&raw_q);
        sql.push_str("SELECT p.id, p.course_code, p.course_title, p.department, p.program, p.semester, p.year, p.exam_type, p.course_category, p.course_level, p.original_path ");
        sql.push_str("FROM papers p ");
        sql.push_str("JOIN papers_fts fts ON p.id = fts.rowid ");
        sql.push_str("WHERE papers_fts MATCH ? ");
        bindings.push(fts_query);
    } else {
        sql.push_str("SELECT id, course_code, course_title, department, program, semester, year, exam_type, course_category, course_level, original_path ");
        sql.push_str("FROM papers WHERE 1=1 ");
    }

    if let Some(ref dept) = params.department {
        if !dept.is_empty() && dept != "All" {
            sql.push_str(" AND p.department = ? ");
            bindings.push(dept.clone());
        }
    }

    if let Some(ref prog) = params.program {
        if !prog.is_empty() && prog != "All" {
            sql.push_str(" AND p.program = ? ");
            bindings.push(prog.clone());
        }
    }

    if let Some(ref sem) = params.semester {
        if !sem.is_empty() && sem != "All" {
            sql.push_str(" AND p.semester = ? ");
            bindings.push(sem.clone());
        }
    }

    if let Some(ref yr) = params.year {
        if !yr.is_empty() && yr != "All" {
            sql.push_str(" AND p.year = ? ");
            bindings.push(yr.clone());
        }
    }

    if let Some(ref cat) = params.category {
        if !cat.is_empty() && cat != "All" {
            sql.push_str(" AND p.course_category = ? ");
            bindings.push(cat.clone());
        }
    }

    if has_text_q {
        sql.push_str(" ORDER BY bm25(papers_fts, 10.0, 5.0, 2.0) ASC LIMIT ?");
    } else {
        sql.push_str(" ORDER BY year DESC, course_code ASC LIMIT ?");
    }

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rusqlite_params: Vec<&dyn rusqlite::ToSql> = Vec::new();
    for b in &bindings {
        rusqlite_params.push(b);
    }
    rusqlite_params.push(&limit);

    let rows = stmt
        .query_map(rusqlite_params.as_slice(), |row| {
            let orig_path: String = row.get(10)?;
            let rel = orig_path
                .strip_prefix(state.indexed_root.to_str().unwrap_or_default())
                .unwrap_or(&orig_path)
                .trim_start_matches('/')
                .to_string();

            Ok(PaperRecord {
                id: row.get(0)?,
                course_code: row.get(1)?,
                course_title: row.get(2)?,
                department: row.get(3)?,
                program: row.get(4)?,
                semester: row.get(5)?,
                year: row.get(6)?,
                exam_type: row.get(7)?,
                course_category: row.get(8)?,
                course_level: row.get(9)?,
                relative_path: rel,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut records = Vec::new();
    for r in rows {
        if let Ok(rec) = r {
            records.push(rec);
        }
    }

    Ok(Json(records))
}

async fn handle_facets(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FacetResponse>, (StatusCode, String)> {
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
    let progs = fetch_facet("SELECT program, count(*) FROM papers GROUP BY program ORDER BY count(*) DESC");
    let yrs = fetch_facet("SELECT year, count(*) FROM papers GROUP BY year ORDER BY year DESC");
    let cats = fetch_facet("SELECT course_category, count(*) FROM papers GROUP BY course_category ORDER BY count(*) DESC");

    Ok(Json(FacetResponse {
        departments: depts,
        programs: progs,
        years: yrs,
        categories: cats,
        total_papers,
    }))
}

async fn handle_pdf(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let rel_path = match params.get("path") {
        Some(p) => p,
        None => return (StatusCode::BAD_REQUEST, "Missing path parameter").into_response(),
    };

    let full_path = if rel_path.starts_with('/') {
        PathBuf::from(rel_path)
    } else {
        state.indexed_root.join(rel_path)
    };

    if !full_path.exists() {
        return (StatusCode::NOT_FOUND, "PDF File Not Found").into_response();
    }

    match tokio_fs::read(&full_path).await {
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
