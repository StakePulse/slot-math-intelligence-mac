//! DOCUMENT LIBRARY : import, classement, extraction de texte et de valeurs À VALIDER.
//!
//! - Un document est identifié par son SHA-256 : réimporter le même fichier ne crée pas de doublon.
//! - Il est rangé dans `DOCUMENTS/<PDF|SCREENSHOTS|EXCEL|OTHER>` ; un fichier déposé à la main dans
//!   `DOCUMENTS/INBOX` (ou n'importe où sous `DOCUMENTS`) est intégré par [`sync`] sans rien casser.
//! - Texte : PDF (pdf-extract, sinon `pdftotext` s'il est installé), CSV / TXT / JSON / MD, Excel
//!   (calamine). Images : AUCUNE reconnaissance de caractères n'est installée sur ce poste — elles sont
//!   classées, et les valeurs se saisissent à la main (statut « À VALIDER » comme les autres).
//! - Les valeurs repérées (RTP, hit rate, max win, « 1 sur N », écart-type, prix…) sont des
//!   **candidats** : statut « À VALIDER » tant qu'un humain ne les a pas confirmés. Elles ne sont
//!   jamais utilisées dans un calcul avant validation.

use crate::db::Db;
use crate::Paths;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DocumentRow {
    pub id: i64,
    pub filename: String,
    pub kind: String,
    pub category: String,
    pub path: String,
    pub size: i64,
    pub imported_at: String,
    pub pages: Option<i64>,
    pub text_status: String,
    pub note: String,
    pub tags: String,
    pub is_lut: bool,
    pub facts_total: i64,
    pub facts_to_validate: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Fact {
    pub id: i64,
    pub document_id: i64,
    pub key: String,
    pub label: String,
    pub value: String,
    pub value_num: Option<f64>,
    pub page: Option<i64>,
    pub context: String,
    pub status: String,
    pub validated_value: Option<String>,
}

pub fn kind_of(path: &Path) -> (&'static str, &'static str) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "pdf" => ("pdf", "PDF"),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => ("image", "SCREENSHOTS"),
        "xlsx" | "xlsm" | "xls" | "ods" => ("excel", "EXCEL"),
        "csv" => ("csv", "OTHER"),
        "json" => ("json", "OTHER"),
        "txt" | "md" => ("text", "OTHER"),
        "zip" => ("archive", "OTHER"),
        _ => ("other", "OTHER"),
    }
}

fn sha256(path: &Path) -> anyhow::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

/// Texte d'un document, découpé en pages quand c'est possible. `None` : pas de texte extractible.
pub fn extract_text(path: &Path, kind: &str) -> (Option<Vec<String>>, String) {
    match kind {
        "pdf" => {
            // 1) pdftotext (mise en page et pages conservées) s'il est installé ; 2) pdf-extract.
            let text = pdftotext_candidates().into_iter().find_map(|exe| {
                let mut cmd = std::process::Command::new(&exe);
                cmd.arg("-layout").arg("-enc").arg("UTF-8").arg(path).arg("-");
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x0800_0000);
                }
                let out = cmd.output().ok()?;
                let t = String::from_utf8_lossy(&out.stdout).to_string();
                (out.status.success() && t.trim().len() > 20).then_some(t)
            });
            let text = text.or_else(|| match std::panic::catch_unwind(|| pdf_extract::extract_text(path)) {
                Ok(Ok(t)) if t.trim().len() > 20 => Some(t),
                _ => None,
            });
            match text {
                Some(t) => {
                    let pages: Vec<String> = t.split('\u{c}').map(String::from).collect();
                    (Some(pages), "TEXTE EXTRAIT".into())
                }
                None => (None, "AUCUN TEXTE (PDF image : saisie manuelle)".into()),
            }
        }
        "csv" | "json" | "text" => match std::fs::read(path) {
            Ok(b) => {
                let t = String::from_utf8_lossy(&b[..b.len().min(8 * 1024 * 1024)]).to_string();
                (Some(vec![t]), "TEXTE LU".into())
            }
            Err(e) => (None, format!("ERREUR : {e}")),
        },
        "excel" => {
            use calamine::{open_workbook_auto, Data, Reader};
            match open_workbook_auto(path) {
                Ok(mut wb) => {
                    let mut pages = Vec::new();
                    for name in wb.sheet_names().to_owned() {
                        if let Ok(range) = wb.worksheet_range(&name) {
                            let mut t = format!("# {name}\n");
                            for row in range.rows().take(20_000) {
                                let cells: Vec<String> = row
                                    .iter()
                                    .map(|c| match c {
                                        Data::Empty => String::new(),
                                        other => other.to_string(),
                                    })
                                    .collect();
                                t.push_str(&cells.join("\t"));
                                t.push('\n');
                            }
                            pages.push(t);
                        }
                    }
                    (Some(pages), "FEUILLES LUES".into())
                }
                Err(e) => (None, format!("ERREUR EXCEL : {e}")),
            }
        }
        "image" => (None, "IMAGE (pas d'OCR installé : saisie manuelle à valider)".into()),
        _ => (None, "NON ANALYSÉ".into()),
    }
}

/// Où chercher `pdftotext` (Poppler). Une application macOS lancée depuis le Finder n'hérite pas du
/// PATH du terminal : les emplacements de Homebrew (Apple Silicon puis Intel) et de MacPorts sont
/// donc essayés explicitement. Sans `pdftotext`, l'extracteur Rust `pdf-extract` prend le relais.
fn pdftotext_candidates() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("pdftotext")];
    #[cfg(windows)]
    let known: &[&str] = &["C:\\Program Files\\Git\\mingw64\\bin\\pdftotext.exe", "C:\\Program Files (x86)\\Git\\mingw64\\bin\\pdftotext.exe", "C:\\Program Files\\poppler\\Library\\bin\\pdftotext.exe"];
    #[cfg(not(windows))]
    let known: &[&str] = &["/opt/homebrew/bin/pdftotext", "/usr/local/bin/pdftotext", "/opt/local/bin/pdftotext", "/usr/bin/pdftotext"];
    for p in known.iter().rev().map(PathBuf::from) {
        if p.exists() {
            v.insert(0, p);
        }
    }
    v
}

/// Réanalyse le texte et les valeurs d'un document (après installation d'un meilleur extracteur, par ex.).
/// Les valeurs déjà validées ou rejetées par un humain sont conservées.
pub fn reanalyze(db: &Db, id: i64) -> anyhow::Result<usize> {
    let (path, kind): (String, String) = db.conn().query_row("SELECT path, kind FROM documents WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let (pages, status) = extract_text(Path::new(&path), &kind);
    let text = pages.as_ref().map(|p| p.join("\u{c}"));
    let c = db.conn();
    c.execute("UPDATE documents SET pages = ?2, text_status = ?3, text = ?4 WHERE id = ?1", params![id, pages.as_ref().map(|p| p.len() as i64), status, text])?;
    c.execute("DELETE FROM extracted_facts WHERE document_id = ?1 AND status = 'À VALIDER' AND context NOT LIKE 'saisie manuelle%'", params![id])?;
    let mut n = 0;
    if let Some(p) = &pages {
        for (key, label, value, num, page, ctx) in find_facts(p) {
            c.execute(
                "INSERT INTO extracted_facts(document_id, key, label, value, value_num, page, context, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'À VALIDER')",
                params![id, key, label, value, num, page, ctx],
            )?;
            n += 1;
        }
    }
    Ok(n)
}

struct Pattern {
    key: &'static str,
    label: &'static str,
    re: regex::Regex,
    group: usize,
}

fn patterns() -> Vec<Pattern> {
    let p = |key, label, re: &str, group| Pattern { key, label, re: regex::Regex::new(re).unwrap(), group };
    vec![
        p("rtp", "RTP", r"(?i)\b(?:RTP|return to player)\b[^\n%]{0,40}?(\d{2}[.,]\d{1,4})\s?%", 1),
        p("hit_rate", "Hit rate", r"(?i)\bhit\s*(?:rate|frequency|freq\.?)\b[^\n%]{0,40}?(\d{1,3}[.,]\d{1,4})\s?%", 1),
        p("max_win", "Max win (×)", r"(?i)\bmax(?:imum)?\s*win\b[^\n\d]{0,40}?(\d{1,3}(?:[ ,\u{202f}\u{a0}]\d{3})*(?:[.,]\d+)?)\s?[x×]", 1),
        p("one_in", "Fréquence « 1 sur N »", r"(?i)\b1\s+(?:in|sur)\s+(\d{1,3}(?:[ ,\u{202f}\u{a0}]\d{3})*(?:[.,]\d+)?)", 1),
        p("sd", "Écart-type / volatilité", r"(?i)\b(?:standard deviation|écart[- ]type|SD\s*/\s*price|volatility \(SD)\b[^\n\d]{0,30}?(\d+(?:[.,]\d+)?)", 1),
        p("price", "Prix d'un mode (×)", r"(?i)\b(?:price|prix|cost|coût)\b[^\n\d]{0,15}?(\d+(?:[.,]\d+)?)\s?[x×]", 1),
        p("bonus_freq", "Fréquence des bonus", r"(?i)\bbonus\b[^\n]{0,40}?\b1\s+(?:in|sur)\s+(\d{1,3}(?:[ ,\u{202f}\u{a0}]\d{3})*(?:[.,]\d+)?)", 1),
        p("percent", "Pourcentage (contexte à lire)", r"(?i)(?:median|médiane|P\d{2}|percentile)[^\n%]{0,30}?(\d+(?:[.,]\d+)?)\s?[x×%]", 1),
    ]
}

fn parse_num(s: &str) -> Option<f64> {
    let t: String = s.chars().filter(|c| !matches!(c, ' ' | '\u{202f}' | '\u{a0}')).collect();
    // « 6,243,175 » (milliers anglais) ou « 96,70 » (décimale française).
    let t = if t.matches(',').count() >= 1 && t.split(',').skip(1).all(|g| g.len() == 3) && !t.contains('.') {
        t.replace(',', "")
    } else {
        t.replace(',', ".")
    };
    t.parse::<f64>().ok()
}

/// Candidats repérés dans le texte (au plus 400 par document, sans doublon clé + valeur + page).
pub fn find_facts(pages: &[String]) -> Vec<(String, String, String, Option<f64>, Option<i64>, String)> {
    let pats = patterns();
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (pi, page) in pages.iter().enumerate() {
        for p in &pats {
            for cap in p.re.captures_iter(page) {
                let Some(m) = cap.get(p.group) else { continue };
                let value = m.as_str().trim().to_string();
                if !seen.insert((p.key, value.clone(), pi)) {
                    continue;
                }
                let whole = cap.get(0).unwrap();
                let a = page[..whole.start()].char_indices().rev().nth(60).map_or(0, |(i, _)| i);
                let b = page[whole.end()..].char_indices().nth(40).map_or(page.len(), |(i, _)| whole.end() + i);
                let ctx: String = page[a..b].split_whitespace().collect::<Vec<_>>().join(" ");
                out.push((p.key.to_string(), p.label.to_string(), value.clone(), parse_num(&value), Some(pi as i64 + 1), ctx));
                if out.len() >= 400 {
                    return out;
                }
            }
        }
    }
    out
}

fn looks_like_lut(text: &str) -> bool {
    let mut n = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()).take(6) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() == 3 && parts.iter().all(|p| p.trim().parse::<u64>().is_ok()) {
            n += 1;
        }
    }
    n >= 3
}

/// Importe (ou retrouve) un fichier. `copy` : copier dans DOCUMENTS si le fichier est ailleurs.
pub fn import(db: &Db, paths: &Paths, src: &Path, copy: bool) -> anyhow::Result<(i64, bool)> {
    let hash = sha256(src)?;
    if let Some(id) = db.conn().query_row("SELECT id FROM documents WHERE sha256 = ?1", params![hash], |r| r.get(0)).optional()? {
        return Ok((id, false));
    }
    let (kind, category) = kind_of(src);
    let filename = src.file_name().and_then(|n| n.to_str()).unwrap_or("document").to_string();
    let inside = src.canonicalize().ok().zip(paths.documents.canonicalize().ok()).is_some_and(|(a, b)| a.starts_with(&b));
    let in_inbox = src.canonicalize().ok().zip(paths.documents.join("INBOX").canonicalize().ok()).is_some_and(|(a, b)| a.starts_with(&b));
    let dest: PathBuf = if (copy && !inside) || in_inbox {
        let dir = paths.documents.join(category);
        std::fs::create_dir_all(&dir)?;
        let mut d = dir.join(&filename);
        let mut k = 1;
        while d.exists() {
            let stem = Path::new(&filename).file_stem().and_then(|s| s.to_str()).unwrap_or("document");
            let ext = Path::new(&filename).extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
            d = dir.join(format!("{stem}_{k}{ext}"));
            k += 1;
        }
        if in_inbox {
            std::fs::rename(src, &d).or_else(|_| std::fs::copy(src, &d).map(|_| ()))?;
        } else {
            std::fs::copy(src, &d)?;
        }
        d
    } else {
        src.to_path_buf()
    };
    let size = std::fs::metadata(&dest)?.len() as i64;
    let (pages, status) = extract_text(&dest, kind);
    let text = pages.as_ref().map(|p| p.join("\u{c}"));
    let is_lut = kind == "csv" && text.as_deref().is_some_and(looks_like_lut);
    let id = {
        let c = db.conn();
        c.execute(
            "INSERT INTO documents(sha256, filename, kind, category, path, size, imported_at, pages, text_status, text, is_lut)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![hash, filename, kind, category, dest.display().to_string(), size, crate::now(), pages.as_ref().map(|p| p.len() as i64), status, text, is_lut as i64],
        )?;
        c.last_insert_rowid()
    };
    if let Some(p) = &pages {
        if !is_lut {
            let facts = find_facts(p);
            let c = db.conn();
            for (key, label, value, num, page, ctx) in facts {
                c.execute(
                    "INSERT INTO extracted_facts(document_id, key, label, value, value_num, page, context, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'À VALIDER')",
                    params![id, key, label, value, num, page, ctx],
                )?;
            }
        }
    }
    db.log("document", &format!("{filename} importé ({status})"));
    Ok((id, true))
}

/// Intègre tout fichier présent sous DOCUMENTS et pas encore enregistré (INBOX compris).
pub fn sync(db: &Db, paths: &Paths) -> anyhow::Result<(usize, Vec<String>)> {
    let mut added = 0;
    let mut errors = Vec::new();
    let mut stack = vec![paths.documents.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| !n.starts_with('.') && !n.starts_with("~$")) {
                match import(db, paths, &p, false) {
                    Ok((_, true)) => added += 1,
                    Ok(_) => {}
                    Err(e) => errors.push(format!("{} : {e}", p.display())),
                }
            }
        }
    }
    Ok((added, errors))
}

pub fn list(db: &Db) -> anyhow::Result<Vec<DocumentRow>> {
    let c = db.conn();
    let mut st = c.prepare(
        "SELECT d.id, d.filename, d.kind, d.category, d.path, d.size, d.imported_at, d.pages, d.text_status, d.note, d.tags, d.is_lut,
                (SELECT COUNT(*) FROM extracted_facts f WHERE f.document_id = d.id),
                (SELECT COUNT(*) FROM extracted_facts f WHERE f.document_id = d.id AND f.status = 'À VALIDER')
         FROM documents d ORDER BY d.imported_at DESC, d.id DESC",
    )?;
    let rows = st
        .query_map([], |r| {
            Ok(DocumentRow {
                id: r.get(0)?,
                filename: r.get(1)?,
                kind: r.get(2)?,
                category: r.get(3)?,
                path: r.get(4)?,
                size: r.get(5)?,
                imported_at: r.get(6)?,
                pages: r.get(7)?,
                text_status: r.get(8)?,
                note: r.get(9)?,
                tags: r.get(10)?,
                is_lut: r.get::<_, i64>(11)? != 0,
                facts_total: r.get(12)?,
                facts_to_validate: r.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn facts(db: &Db, document_id: i64) -> anyhow::Result<Vec<Fact>> {
    let c = db.conn();
    let mut st = c.prepare("SELECT id, document_id, key, label, value, value_num, page, context, status, validated_value FROM extracted_facts WHERE document_id = ?1 ORDER BY page, id")?;
    let rows = st
        .query_map(params![document_id], |r| {
            Ok(Fact {
                id: r.get(0)?,
                document_id: r.get(1)?,
                key: r.get(2)?,
                label: r.get(3)?,
                value: r.get(4)?,
                value_num: r.get(5)?,
                page: r.get(6)?,
                context: r.get(7)?,
                status: r.get(8)?,
                validated_value: r.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Décision humaine sur une valeur : « VALIDÉ » (avec valeur corrigée éventuelle) ou « REJETÉ ».
pub fn review_fact(db: &Db, id: i64, status: &str, corrected: Option<&str>) -> anyhow::Result<()> {
    if !matches!(status, "VALIDÉ" | "REJETÉ" | "À VALIDER") {
        anyhow::bail!("statut inconnu : {status}");
    }
    db.conn().execute("UPDATE extracted_facts SET status = ?2, validated_at = ?3, validated_value = ?4 WHERE id = ?1", params![id, status, crate::now(), corrected])?;
    Ok(())
}

/// Saisie manuelle d'une valeur (images, PDF sans texte) : elle entre aussi « À VALIDER ».
pub fn add_manual_fact(db: &Db, document_id: i64, key: &str, label: &str, value: &str, page: Option<i64>, context: &str) -> anyhow::Result<i64> {
    let c = db.conn();
    c.execute(
        "INSERT INTO extracted_facts(document_id, key, label, value, value_num, page, context, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'À VALIDER')",
        params![document_id, key, label, value, parse_num(value), page, format!("saisie manuelle — {context}")],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn update_meta(db: &Db, id: i64, note: &str, tags: &str) -> anyhow::Result<()> {
    db.conn().execute("UPDATE documents SET note = ?2, tags = ?3 WHERE id = ?1", params![id, note, tags])?;
    Ok(())
}

pub fn text(db: &Db, id: i64) -> anyhow::Result<Option<String>> {
    Ok(db.conn().query_row("SELECT text FROM documents WHERE id = ?1", params![id], |r| r.get(0))?)
}

pub fn search(db: &Db, q: &str) -> anyhow::Result<Vec<(i64, String, String)>> {
    let like = format!("%{}%", q.replace('%', ""));
    let c = db.conn();
    let mut st = c.prepare("SELECT id, filename, substr(text, max(1, instr(lower(text), lower(?1)) - 80), 200) FROM documents WHERE text LIKE ?2 OR filename LIKE ?2 OR note LIKE ?2 OR tags LIKE ?2 LIMIT 50")?;
    let rows = st.query_map(params![q, like], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, Option<String>>(2)?.unwrap_or_default())))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_typical_values() {
        let page = "Mode BASE: RTP 96.70% with hit rate 22.00% ; Max win 50,000× ; bonus 1 in 175 ; SD / price 45.72".to_string();
        let f = find_facts(&[page]);
        let get = |k: &str| f.iter().find(|x| x.0 == k).map(|x| x.3);
        assert_eq!(get("rtp"), Some(Some(96.70)));
        assert_eq!(get("hit_rate"), Some(Some(22.0)));
        assert_eq!(get("max_win"), Some(Some(50_000.0)));
        assert_eq!(get("bonus_freq"), Some(Some(175.0)));
    }

    #[test]
    fn number_parsing() {
        assert_eq!(parse_num("6,243,175"), Some(6_243_175.0));
        assert_eq!(parse_num("96,70"), Some(96.70));
        assert_eq!(parse_num("1\u{202f}500"), Some(1500.0));
    }

    #[test]
    fn lut_detection() {
        assert!(looks_like_lut("0,10,0\n1,5,150\n2,5,150\n"));
        assert!(!looks_like_lut("a,b,c\nx,y,z\n"));
    }
}
