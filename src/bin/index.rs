//! Amrita PDF Metadata Extraction, Restructuring & Indexing Tool (`amrita-index`)
//!
//! Scans downloaded exam paper PDFs, uses `pdftotext -layout` to extract course codes,
//! titles, programs, semesters, years, and exam types, then restructures them into a clean,
//! highly searchable library with .meta sidecars, index.tsv, index.json, index.db (SQLite FTS5), and fzf support.

use std::{
    collections::HashMap,
    fs::{self as std_fs, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use clap::Parser;
use rayon::prelude::*;
use rusqlite::{params, Connection};
use sanitize_filename::sanitize;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(name = "amrita-index", about = "Layout-aware indexer & restructurer for Amrita Exam Papers", version)]
struct Args {
    /// Root directory of raw downloads
    #[arg(long, default_value = "/run/media/anuruprkris/DATA/amrita-exam-papers")]
    src: PathBuf,

    /// Destination root directory for indexed library
    #[arg(long, default_value = "/run/media/anuruprkris/DATA/amrita-exam-papers-indexed")]
    dest: PathBuf,

    /// Dry run mode: output rename_proposal.csv without creating/copying files
    #[arg(long)]
    dry_run: bool,

    /// Number of parallel threads to use for parsing
    #[arg(long, default_value_t = 16)]
    threads: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct PdfMeta {
    original_path: String,
    course_code: String,
    course_title: String,
    department: String,
    program: String,
    semester: String,
    year: String,
    exam_type: String,
    course_level: String,
    course_category: String,
    sha256: String,
    confidence: String,
}

fn get_current_year() -> i32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    (1970 + (secs / 31556926)) as i32
}

fn sanitize_text(s: &str) -> String {
    s.replace('\t', " ")
        .replace('\n', " ")
        .replace('\r', " ")
        .trim()
        .to_string()
}

fn sanitize_title(raw: &str) -> String {
    let text = sanitize_text(raw);
    
    let cut_markers = [
        "Time:", "Duration:", "Maximum:", "Marks:", "Marks",
        "(Common to", "branches please", "Commented", "Comment",
        "Roll No", "Register No", "Reg No", "Slot", "Max."
    ];
    
    let mut clean = text.as_str();
    for marker in cut_markers {
        if let Some(pos) = clean.find(marker) {
            clean = &clean[..pos];
        }
    }

    let clean = clean.split_whitespace().collect::<Vec<&str>>().join(" ");
    let clean = clean
        .trim_matches(|c: char| c == '-' || c == ':' || c == '[' || c == ']' || c == '(' || c == ')' || c == '_' || c.is_whitespace())
        .to_string();

    if clean.is_empty() {
        "UNKNOWN".to_string()
    } else {
        clean.chars().take(60).collect()
    }
}

fn slugify(s: &str) -> String {
    let s = sanitize(s.trim());
    let s = s.replace(' ', "_").replace("/", "-");
    let s = s.trim_matches('_').to_string();
    if s.is_empty() || s == "UNKNOWN" { "unknown".to_string() } else { s.chars().take(60).collect() }
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn classify_department(code: &str, title: &str, path_str: &str) -> &'static str {
    let c = code.to_uppercase();
    let t = title.to_lowercase();
    let p = path_str.to_lowercase();

    if c.contains("CSE") || c.contains("CSC") || c.contains("CSA") || c.contains("CS6") || c.contains("CS7") || c.contains("CS8") || c.contains("CYS") || c.contains("AIE") || c.contains("CCE") || c.contains("CA3") || c.contains("CA4") || t.contains("computer") || t.contains("software") || t.contains("data structure") || t.contains("algorithm") || t.contains("cyber") || t.contains("database") {
        "Computer Science & Engineering"
    } else if c.contains("ECE") || c.contains("ELC") || c.contains("EIE") || c.contains("VL6") || c.contains("VL7") || c.contains("VL8") || c.contains("EC1") || c.contains("EC2") || c.contains("EC8") || t.contains("electronics") || t.contains("signal") || t.contains("vlsi") || t.contains("embedded") {
        "Electronics & Communication"
    } else if c.contains("EEE") || c.contains("AEE") || c.contains("ES6") || c.contains("PE6") || c.contains("PE7") || c.contains("CIE") || c.contains("RE7") || t.contains("electrical") || t.contains("power") || t.contains("high voltage") {
        "Electrical & Electronics"
    } else if c.contains("MEC") || c.contains("MEE") || c.contains("AES") || c.contains("AT6") || c.contains("ME7") || c.contains("ME8") || c.contains("WME") || t.contains("mechanical") || t.contains("thermal") || t.contains("combustion") || t.contains("automotive") {
        "Mechanical & Aerospace"
    } else if c.contains("CVL") || c.contains("CE7") || c.contains("CEN") || c.contains("ED6") || c.contains("SC7") || t.contains("civil") || t.contains("structural") {
        "Civil Engineering"
    } else if c.contains("CHE") || c.contains("CH6") || c.contains("SC6") || t.contains("chemical engineering") {
        "Chemical Engineering"
    } else if c.contains("MAT") || c.contains("MA6") || c.contains("MA7") || c.contains("MA8") || c.contains("MA9") || c.contains("MAL") || t.contains("mathematics") || t.contains("calculus") || t.contains("algebra") || t.contains("statistic") {
        "Mathematics & Statistics"
    } else if c.contains("PHY") || c.contains("PH6") || c.contains("PH7") || t.contains("physics") {
        "Physical Sciences"
    } else if c.contains("CHY") || c.contains("CY6") || c.contains("CY7") || c.contains("CY4") || t.contains("chemistry") {
        "Chemical Sciences"
    } else if c.contains("BIO") || c.contains("BM6") || c.contains("BM7") || c.contains("BM8") || c.contains("BMT") || c.contains("HOR") || c.contains("GPB") || c.contains("SS8") || t.contains("bio") || t.contains("medical") {
        "Biotechnology & Health"
    } else if c.contains("CMJ") || c.contains("COM") || c.contains("DDA") || t.contains("media") || t.contains("journalism") || t.contains("communication") {
        "Media & Communication"
    } else if c.contains("HUM") || c.contains("ELL") || c.contains("ENG") || c.contains("TAM") || c.contains("GER") || c.contains("FRE") || c.contains("MAL") || c.contains("FSN") || c.contains("SOC") || c.contains("SWK") || c.contains("SW") || p.contains("social work") || t.contains("english") || t.contains("social") || t.contains("culture") {
        "Humanities & Social Sciences"
    } else {
        "General & Interdisciplinary"
    }
}

fn classify_course_category(code: &str, title: &str) -> &'static str {
    let c = code.to_uppercase();
    let t = title.to_lowercase();

    if t.contains("lab") || t.contains("laboratory") || t.contains("practical") || t.contains("workshop") || c.ends_with('L') {
        "Lab"
    } else if c.starts_with("OEL") || t.contains("open elective") || t.contains("science of food") || t.contains("human values") {
        "OpenElective"
    } else if c.contains("35") || c.contains("36") || c.contains("45") || c.contains("46") || c.contains("71") || c.contains("72") || t.contains("elective") || t.contains("specialization") {
        "DeptElective"
    } else {
        "Core"
    }
}

fn classify_course_level(code: &str) -> &'static str {
    let digits: String = code.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() >= 3 {
        let first = digits.chars().next().unwrap();
        match first {
            '1' => "100-Level Introductory",
            '2' => "200-Level Core",
            '3' => "300-Level Intermediate",
            '4' => "400-Level Senior Electives",
            '5' | '6' => "500-600 Level Master's",
            '7' | '8' => "700-800 Level Advanced PhD",
            _ => "General Level",
        }
    } else {
        "General Level"
    }
}

/// Extract metadata from PDF header text via `pdftotext -layout`
fn extract_pdf_metadata(pdf_path: &Path) -> PdfMeta {
    let mut meta = PdfMeta {
        original_path: pdf_path.to_string_lossy().to_string(),
        confidence: "low".to_string(),
        ..Default::default()
    };

    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg("-f").arg("1")
        .arg("-l").arg("1")
        .arg(pdf_path)
        .arg("-")
        .output();

    let text = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return meta_from_path(pdf_path, &mut meta),
    };

    let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();

    // Limit header metadata parsing (degree, semester, exam_type, year) to top 15 lines of Page 1
    let header_lines = lines.iter().take(15);

    for line in header_lines {
        let l_lower = line.to_lowercase();

        if l_lower.contains("b.tech") || l_lower.contains("bachelor of technology") { meta.program = "B.Tech".to_string(); }
        else if l_lower.contains("m.tech") || l_lower.contains("master of technology") { meta.program = "M.Tech".to_string(); }
        else if l_lower.contains("mca") || l_lower.contains("master of computer applications") { meta.program = "MCA".to_string(); }
        else if l_lower.contains("ph.d") || l_lower.contains("phd") { meta.program = "PhD".to_string(); }
        else if l_lower.contains("int. m.sc") || l_lower.contains("integrated msc") || l_lower.contains("integrated m.sc") || l_lower.contains("integrated ma") { meta.program = "Integrated M.Sc.".to_string(); }
        else if l_lower.contains("msw") || l_lower.contains("master of social work") { meta.program = "MSW".to_string(); }

        if l_lower.contains("first semester") || l_lower.contains("1st semester") || l_lower.contains("i sem ") || l_lower.contains("i semester") { meta.semester = "Sem01".to_string(); }
        else if l_lower.contains("second semester") || l_lower.contains("2nd semester") || l_lower.contains("ii sem ") || l_lower.contains("ii semester") { meta.semester = "Sem02".to_string(); }
        else if l_lower.contains("third semester") || l_lower.contains("3rd semester") || l_lower.contains("iii sem ") || l_lower.contains("iii semester") { meta.semester = "Sem03".to_string(); }
        else if l_lower.contains("fourth semester") || l_lower.contains("4th semester") || l_lower.contains("iv sem ") || l_lower.contains("iv semester") { meta.semester = "Sem04".to_string(); }
        else if l_lower.contains("fifth semester") || l_lower.contains("5th semester") || l_lower.contains("v sem ") || l_lower.contains("v semester") { meta.semester = "Sem05".to_string(); }
        else if l_lower.contains("sixth semester") || l_lower.contains("6th semester") || l_lower.contains("vi sem ") || l_lower.contains("vi semester") { meta.semester = "Sem06".to_string(); }
        else if l_lower.contains("seventh semester") || l_lower.contains("7th semester") || l_lower.contains("vii sem ") || l_lower.contains("vii semester") { meta.semester = "Sem07".to_string(); }
        else if l_lower.contains("eighth semester") || l_lower.contains("8th semester") || l_lower.contains("viii sem ") || l_lower.contains("viii semester") { meta.semester = "Sem08".to_string(); }
        else if l_lower.contains("ninth semester") || l_lower.contains("9th semester") || l_lower.contains("ix sem ") || l_lower.contains("ix semester") { meta.semester = "Sem09".to_string(); }
        else if l_lower.contains("tenth semester") || l_lower.contains("10th semester") || l_lower.contains("x sem ") || l_lower.contains("x semester") { meta.semester = "Sem10".to_string(); }

        if l_lower.contains("first assessment") || l_lower.contains("ass i ") || l_lower.contains("i ass") { meta.exam_type = "Ass1".to_string(); }
        else if l_lower.contains("second assessment") || l_lower.contains("ass ii ") || l_lower.contains("ii ass") { meta.exam_type = "Ass2".to_string(); }
        else if l_lower.contains("third assessment") || l_lower.contains("ass iii ") || l_lower.contains("iii ass") { meta.exam_type = "Ass3".to_string(); }
        else if l_lower.contains("mid-term") || l_lower.contains("mid term") || l_lower.contains("midterm") { meta.exam_type = "MidTerm".to_string(); }
        else if l_lower.contains("degree examination") || l_lower.contains("end semester") || l_lower.contains("end sem") { meta.exam_type = "EndSem".to_string(); }

        for token in line.split_whitespace() {
            let clean = token.trim_matches(|c: char| !c.is_ascii_digit());
            if clean.len() == 4 {
                if let Ok(y) = clean.parse::<i32>() {
                    let max_year = get_current_year() + 1;
                    if y >= 2000 && y <= max_year {
                        meta.year = clean.to_string();
                    }
                }
            }
        }
    }

    let re_code = regex::Regex::new(r"(?i)\b([0-9]{2}[A-Z]{2,6}[0-9]{3,4}|[A-Z]{2,6}[0-9]{3,4}|[A-Z]{2,4}\s?[0-9]{3,4})").unwrap();
    for line in &lines {
        if let Some(mat) = re_code.find(line) {
            let code = mat.as_str();
            meta.course_code = code.to_string();
            
            let remainder = &line[mat.end()..];
            let clean_title = sanitize_title(remainder);

            if !clean_title.is_empty() && clean_title != "UNKNOWN" && clean_title.len() > 2 {
                meta.course_title = clean_title;
                meta.confidence = "high".to_string();
                break;
            }
        }
    }

    meta_from_path(pdf_path, &mut meta);

    meta
}

fn meta_from_path(pdf_path: &Path, meta: &mut PdfMeta) -> PdfMeta {
    let path_str = pdf_path.to_string_lossy();
    let fname = pdf_path.file_name().unwrap_or_default().to_string_lossy();

    let re_code = regex::Regex::new(r"(?i)\b([0-9]{2}[A-Z]{2,6}[0-9]{3,4}|[A-Z]{2,6}[0-9]{3,4}|[A-Z]{2,4}\s?[0-9]{3,4})").unwrap();

    if meta.course_code.is_empty() {
        if let Some(mat) = re_code.find(&fname) {
            meta.course_code = mat.as_str().to_string();
        } else {
            let first_part = fname.split('.').next().unwrap_or("UNKNOWN");
            let clean_code = first_part.split('_').next().unwrap_or(first_part).trim();
            meta.course_code = if clean_code.is_empty() { "UNKNOWN".to_string() } else { clean_code.to_string() };
        }
    }

    // Clean any residual filename suffix from course_code (e.g. 16MA608_Ass II -> 16MA608)
    if meta.course_code.contains('_') {
        if let Some(clean) = meta.course_code.split('_').next() {
            if !clean.trim().is_empty() {
                meta.course_code = clean.trim().to_string();
            }
        }
    }

    if meta.course_title.is_empty() || meta.course_title == "UNKNOWN" || meta.course_title.eq_ignore_ascii_case(&meta.course_code) {
        let raw_title = fname.replace(&meta.course_code, "").replace(".pdf", "");
        let mut clean = sanitize_title(&raw_title);

        if clean.is_empty() || clean == "UNKNOWN" || clean.eq_ignore_ascii_case(&meta.course_code) {
            if let Some(parent) = pdf_path.parent() {
                let p_name = parent.file_name().unwrap_or_default().to_string_lossy();
                let p_clean = sanitize_title(&p_name);
                if !p_clean.is_empty() && p_clean != "UNKNOWN" {
                    clean = format!("{} ({})", meta.course_code, p_clean);
                } else {
                    clean = format!("{} Examination Paper", meta.course_code);
                }
            } else {
                clean = format!("{} Examination Paper", meta.course_code);
            }
        }
        meta.course_title = clean;
    }

    if meta.program.is_empty() || meta.program == "BTech" || meta.program == "MTech" {
        if path_str.contains("B.Tech") || meta.program == "BTech" { meta.program = "B.Tech".to_string(); }
        else if path_str.contains("M.Tech") || meta.program == "MTech" { meta.program = "M.Tech".to_string(); }
        else if path_str.contains("MCA") { meta.program = "MCA".to_string(); }
        else if path_str.contains("PhD") || path_str.contains("Ph.D") { meta.program = "PhD".to_string(); }
        else if path_str.contains("Integrated MSc") || path_str.contains("Int. M.Sc") || path_str.contains("Integrated") { meta.program = "Integrated M.Sc.".to_string(); }
        else if path_str.contains("PG Diploma") { meta.program = "PG Diploma".to_string(); }
        else if path_str.contains("PG/") || path_str.contains("PG\\") { meta.program = "M.Sc.".to_string(); }
        else if path_str.contains("BA Communi") || path_str.contains("BA Communication") || (path_str.contains("Communication") && !path_str.contains("Electronics")) { meta.program = "B.A. Communication".to_string(); }
        else if path_str.contains("Social Work") || path_str.contains("MSW") { meta.program = "MSW".to_string(); }
        else if path_str.contains("Arts") || path_str.contains("Humanities") { meta.program = "Humanities".to_string(); }
        else if path_str.contains("Science") { meta.program = "M.Sc.".to_string(); }
        else { meta.program = "B.Tech".to_string(); }
    }

    if meta.semester.is_empty() {
        let re_sem = regex::Regex::new(r"(?i)(\d+)(st|nd|rd|th)?\s*Semester").unwrap();
        if let Some(caps) = re_sem.captures(&path_str) {
            if let Ok(num) = caps[1].parse::<u32>() {
                meta.semester = format!("Sem{:02}", num);
            }
        }
        if meta.semester.is_empty() { meta.semester = "SemGeneral".to_string(); }
    }

    let max_year = get_current_year() + 1;
    let re_yr = regex::Regex::new(r"/(19[89]\d|20[0-9]{2})\b|\b(19[89]\d|20[0-9]{2})\b").unwrap();
    if let Some(mat) = re_yr.captures(&path_str) {
        if let Some(m) = mat.get(1).or_else(|| mat.get(2)) {
            if let Ok(y) = m.as_str().parse::<i32>() {
                if y >= 1990 && y <= max_year {
                    meta.year = m.as_str().to_string();
                }
            }
        }
    }
    if meta.year.is_empty() { meta.year = get_current_year().to_string(); }

    if meta.exam_type.is_empty() {
        let l_path = path_str.to_lowercase();
        if l_path.contains("first assessment") || l_path.contains("1st assessment") || l_path.contains("ass i") { meta.exam_type = "Ass1".to_string(); }
        else if l_path.contains("second assessment") || l_path.contains("2nd assessment") || l_path.contains("ass ii") { meta.exam_type = "Ass2".to_string(); }
        else if l_path.contains("third assessment") || l_path.contains("3rd assessment") || l_path.contains("ass iii") { meta.exam_type = "Ass3".to_string(); }
        else if l_path.contains("mid term") || l_path.contains("mid-term") || l_path.contains("midterm") { meta.exam_type = "MidTerm".to_string(); }
        else { meta.exam_type = "EndSem".to_string(); }
    }

    meta.course_code = sanitize_text(&meta.course_code);
    meta.course_title = sanitize_title(&meta.course_title);
    meta.program = sanitize_text(&meta.program);
    meta.semester = sanitize_text(&meta.semester);
    meta.year = sanitize_text(&meta.year);
    meta.exam_type = sanitize_text(&meta.exam_type);

    meta.department = classify_department(&meta.course_code, &meta.course_title, &meta.original_path).to_string();
    meta.course_category = classify_course_category(&meta.course_code, &meta.course_title).to_string();
    meta.course_level = classify_course_level(&meta.course_code).to_string();

    if let Ok(y) = meta.year.parse::<i32>() {
        let max_year = get_current_year() + 1;
        if y < 1990 || y > max_year {
            meta.year = get_current_year().to_string();
        }
    }

    meta.clone()
}

fn collect_pdfs(dir: &Path, pdfs: &mut Vec<PathBuf>) -> Result<()> {
    if dir.is_dir() {
        for entry in std_fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_pdfs(&path, pdfs)?;
            } else if path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) == Some("pdf".to_string()) {
                pdfs.push(path);
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    rayon::ThreadPoolBuilder::new().num_threads(args.threads).build_global()?;

    println!("Scanning PDF files in {}...", args.src.display());
    let mut pdf_files = Vec::new();
    collect_pdfs(&args.src, &mut pdf_files)?;
    println!("Found {} PDF files to process.", pdf_files.len());

    let proposal_file = Arc::new(Mutex::new(BufWriter::new(File::create("rename_proposal.csv")?)));
    writeln!(
        proposal_file.lock().unwrap(),
        "\"original_path\",\"proposed_path\",\"course_code\",\"course_title\",\"department\",\"program\",\"semester\",\"year\",\"exam_type\",\"confidence\""
    )?;

    let sha_map: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let metadata_list: Arc<Mutex<Vec<PdfMeta>>> = Arc::new(Mutex::new(Vec::new()));

    pdf_files.par_iter().for_each(|pdf| {
        let mut meta = extract_pdf_metadata(pdf);
        if let Ok(hash) = sha256_file(pdf) {
            meta.sha256 = hash;
        }

        let title_slug = slugify(&meta.course_title);
        let new_fname = if title_slug == "unknown" || title_slug == meta.course_code.to_lowercase() {
            format!("{}_{}_{}.pdf", meta.course_code, meta.exam_type, meta.year)
        } else {
            format!("{}_{}_{}_{}.pdf", meta.course_code, title_slug, meta.exam_type, meta.year)
        };

        // New Disk Hierarchy: [Department]/[Program]/[Category]/[Year]/[ExamType]/[Filename]
        let dest_rel = PathBuf::from(&meta.department)
            .join(&meta.program)
            .join(&meta.course_category)
            .join(&meta.year)
            .join(&meta.exam_type)
            .join(&new_fname);
        let dest_full = args.dest.join(&dest_rel);

        if let Ok(mut f) = proposal_file.lock() {
            let _ = writeln!(
                f,
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
                pdf.display(),
                dest_full.display(),
                meta.course_code,
                meta.course_title.replace('"', "'"),
                meta.department,
                meta.program,
                meta.semester,
                meta.year,
                meta.exam_type,
                meta.confidence
            );
        }

        if !args.dry_run {
            let parent = dest_full.parent().unwrap();
            let _ = std_fs::create_dir_all(parent);

            if let Ok(mut map) = sha_map.lock() {
                if let Some(existing_dest) = map.get(&meta.sha256) {
                    let _ = std_fs::hard_link(existing_dest, &dest_full);
                } else {
                    if std_fs::copy(pdf, &dest_full).is_ok() {
                        map.insert(meta.sha256.clone(), dest_full.to_string_lossy().to_string());
                    }
                }
            }

            let meta_path = dest_full.with_extension("meta");
            let _ = std_fs::write(meta_path, serde_json::to_string_pretty(&meta).unwrap_or_default());
        }

        if let Ok(mut list) = metadata_list.lock() {
            list.push(meta);
        }
    });

    println!("\nExtraction complete. Building search indices...");

    let metas = metadata_list.lock().unwrap();

    if !args.dry_run {
        std_fs::create_dir_all(&args.dest)?;

        // Write index.json
        let json_path = args.dest.join("index.json");
        std_fs::write(&json_path, serde_json::to_string_pretty(&*metas)?)?;

        // Write index.tsv
        let tsv_path = args.dest.join("index.tsv");
        let mut tsv = BufWriter::new(File::create(tsv_path)?);
        writeln!(tsv, "CourseCode\tCourseTitle\tDepartment\tProgram\tSemester\tYear\tExamType\tCourseCategory\tCourseLevel\tOriginalPath")?;
        for m in metas.iter() {
            writeln!(
                tsv,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                sanitize_text(&m.course_code),
                sanitize_text(&m.course_title),
                sanitize_text(&m.department),
                sanitize_text(&m.program),
                sanitize_text(&m.semester),
                sanitize_text(&m.year),
                sanitize_text(&m.exam_type),
                sanitize_text(&m.course_category),
                sanitize_text(&m.course_level),
                sanitize_text(&m.original_path)
            )?;
        }
        tsv.flush()?;

        // Write index.db (SQLite with FTS5 and B-Tree Indexes)
        let db_path = args.dest.join("index.db");
        if db_path.exists() {
            let _ = std_fs::remove_file(&db_path);
        }
        let mut conn = Connection::open(&db_path)?;
        conn.execute(
            "CREATE TABLE papers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                course_code TEXT NOT NULL,
                course_title TEXT NOT NULL,
                department TEXT NOT NULL,
                program TEXT NOT NULL,
                semester TEXT NOT NULL,
                year TEXT NOT NULL,
                exam_type TEXT NOT NULL,
                course_category TEXT NOT NULL,
                course_level TEXT NOT NULL,
                sha256 TEXT,
                confidence TEXT,
                original_path TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute("CREATE INDEX idx_department ON papers(department);", [])?;
        conn.execute("CREATE INDEX idx_program ON papers(program);", [])?;
        conn.execute("CREATE INDEX idx_category ON papers(course_category);", [])?;
        conn.execute("CREATE INDEX idx_year ON papers(year);", [])?;
        conn.execute("CREATE INDEX idx_code ON papers(course_code);", [])?;
        conn.execute("CREATE INDEX idx_title ON papers(course_title);", [])?;

        conn.execute(
            "CREATE VIRTUAL TABLE papers_fts USING fts5(course_code, course_title, department, program, course_category, year, exam_type)",
            [],
        )?;

        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO papers (course_code, course_title, department, program, semester, year, exam_type, course_category, course_level, sha256, confidence, original_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;
            let mut fts_stmt = tx.prepare(
                "INSERT INTO papers_fts (course_code, course_title, department, program, course_category, year, exam_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;

            for m in metas.iter() {
                let code = sanitize_text(&m.course_code);
                let title = sanitize_text(&m.course_title);
                let dept = sanitize_text(&m.department);
                let prog = sanitize_text(&m.program);
                let sem = sanitize_text(&m.semester);
                let yr = sanitize_text(&m.year);
                let etype = sanitize_text(&m.exam_type);
                let cat = sanitize_text(&m.course_category);
                let lvl = sanitize_text(&m.course_level);
                let orig = sanitize_text(&m.original_path);

                stmt.execute(params![code, title, dept, prog, sem, yr, etype, cat, lvl, m.sha256, m.confidence, orig])?;
                fts_stmt.execute(params![code, title, dept, prog, cat, yr, etype])?;
            }
        }
        tx.commit()?;

        // Write README search guide
        let readme_path = args.dest.join("README.md");
        std_fs::write(readme_path, r#"# Amrita Exam Papers Search Guide

## Disk Structure Hierarchy
`[Department]/[Program]/[CourseCategory]/[Year]/[ExamType]/[CleanFilename].pdf`

## Instant Search via Terminal

### 1. SQLite FTS5 Full-Text Search (`index.db`):
```bash
sqlite3 index.db "SELECT course_code, course_title, department FROM papers_fts WHERE papers_fts MATCH 'Machine Learning';"
```

### 2. Search by Course Title or Code using `rga` (ripgrep-all):
```bash
rga "Digital Signal Processing" /run/media/anuruprkris/DATA/amrita-exam-papers-indexed/
```

### 3. Interactive Terminal Filtering using `fzf`:
```bash
cat index.tsv | fzf --header-lines=1 --with-nth 1,2,3,4,8
```
"#)?;

        println!("✓ Created index.json, index.tsv, index.db, and README.md at {}", args.dest.display());
    }

    println!("✓ Proposal saved to rename_proposal.csv");
    println!("Processing finished successfully.");

    Ok(())
}
