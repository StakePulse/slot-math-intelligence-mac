//! Collecte StakeCruncher — **désactivée par défaut**.
//!
//! Constat du 2026-11-06 (PROJECT/AUDIT.md §3) : les pages publiques du site ne contiennent aucune
//! donnée sans JavaScript ; les données viennent de l'API JSON `/tracker-api/engine/…`, que le
//! `robots.txt` du site interdit aux robots (« private JSON API paths »). Ce module ne s'exécute que
//! si l'utilisateur l'a explicitement activé dans les réglages, après avoir lu cet avertissement
//! (date d'acceptation enregistrée). Il ne contourne rien : pas d'authentification, pas de CAPTCHA,
//! débit lent (≥ 1 s entre requêtes), arrêt sur HTTP 429, User-Agent identifiable, cache local.
//!
//! Ce qu'il collecte : classement (bets et turnover hier / 7 j / 30 j / 90 j / total, joueurs), le
//! catalogue des modes, puis les lookup tables publiées des N slots les plus jouées AU NOMBRE DE BETS
//! sur 30 jours ; chaque table est analysée EXACTEMENT par notre moteur. Toute absence de champ est
//! enregistrée comme « non disponible » ; tout changement de structure de l'API lève une alerte.

use crate::db::Db;
use crate::market::{insert_popularity, lut_path, store_exact_mode, upsert_slot};
use crate::settings::AppSettings;
use crate::Paths;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use smi_core::dist::Distribution;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const API: &str = "https://stakecruncher.com/tracker-api/engine";
pub const REFERER: &str = "https://stakecruncher.com/slots-tracker";
pub const ROBOTS_NOTICE: &str = "Le robots.txt de stakecruncher.com contient « Disallow: /tracker-api/ » (chemins d'API JSON privés). \
Les pages publiques du site n'affichent les données qu'avec JavaScript, en appelant cette même API. Activer la collecte automatique \
est une décision de l'utilisateur ; elle reste lente, identifiable et s'arrête au premier refus du serveur.";

/// Champs attendus d'un jeu du classement ; leur absence est signalée, jamais inventée.
pub const GAME_FIELDS: [&str; 16] = [
    "slug",
    "name",
    "team_name",
    "team_slug",
    "stake_com_slug",
    "turnover_yesterday",
    "bet_count_yesterday",
    "turnover_past_week",
    "bet_count_past_week",
    "turnover_past_30d",
    "bet_count_past_30d",
    "turnover_rolling_90d",
    "bet_count_rolling_90d",
    "bet_count_all_time",
    "current_players_com",
    "peak_players_com",
];

const INSTANT_WORDS: [&str; 10] = ["keno", "dice", "plinko", "mines", "limbo", "crash", "roulette", "blackjack", "baccarat", "hilo"];

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CollectReport {
    pub collection_id: i64,
    pub games_ranked: usize,
    pub slots_selected: usize,
    pub luts_downloaded: usize,
    pub luts_cached: usize,
    pub missing_fields: Vec<String>,
    pub schema_changed: bool,
    pub errors: Vec<String>,
    pub message: String,
}

struct Client {
    agent: ureq::Agent,
    delay: Duration,
}

struct Response {
    status: u16,
    body: Vec<u8>,
    headers: Vec<(String, String)>,
}

impl Client {
    fn new(s: &AppSettings) -> Self {
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(90)).user_agent(&s.collector.user_agent).build();
        Self { agent, delay: Duration::from_millis(s.collector.delay_ms.max(1_000)) }
    }

    fn get(&self, url: &str, cancel: &AtomicBool) -> anyhow::Result<Response> {
        for attempt in 0..3 {
            if cancel.load(Ordering::Relaxed) {
                anyhow::bail!("collecte annulée");
            }
            std::thread::sleep(self.delay);
            match self.agent.get(url).set("Referer", REFERER).call() {
                Ok(r) => {
                    let headers = r.headers_names().into_iter().map(|h| (h.to_lowercase(), r.header(&h).unwrap_or("").to_string())).collect::<Vec<_>>();
                    let status = r.status();
                    let mut body = Vec::new();
                    r.into_reader().take(200 * 1024 * 1024).read_to_end(&mut body)?;
                    let remaining = headers.iter().find(|(k, _)| k == "x-ratelimit-remaining").and_then(|(_, v)| v.parse::<i64>().ok());
                    if remaining.is_some_and(|v| v < 20) {
                        std::thread::sleep(Duration::from_secs(60));
                    }
                    return Ok(Response { status, body, headers });
                }
                Err(ureq::Error::Status(429, _)) => {
                    if attempt == 2 {
                        anyhow::bail!("HTTP 429 (trop de requêtes) répété : collecte arrêtée par politesse");
                    }
                    std::thread::sleep(Duration::from_secs(30 * (attempt + 1)));
                }
                Err(ureq::Error::Status(code, _)) => anyhow::bail!("HTTP {code} sur {url}"),
                Err(e) => anyhow::bail!("réseau : {e}"),
            }
        }
        anyhow::bail!("échec répété sur {url}")
    }
}

fn header<'a>(r: &'a Response, name: &str) -> Option<&'a str> {
    r.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// Signature de structure : clés triées d'un élément.
pub fn signature(v: &serde_json::Value) -> String {
    let mut keys: Vec<&str> = v.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
    keys.sort_unstable();
    keys.join(",")
}

pub fn missing_fields(v: &serde_json::Value) -> Vec<String> {
    GAME_FIELDS.iter().filter(|f| v.get(**f).is_none()).map(|f| f.to_string()).collect()
}

fn num(v: &serde_json::Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}

/// Slug du catalogue correspondant à un jeu du classement (préfixe du studio retiré au besoin).
pub fn catalog_slug(item: &serde_json::Value, catalog: &std::collections::BTreeMap<String, serde_json::Value>) -> Option<String> {
    let s = item["slug"].as_str()?;
    if catalog.contains_key(s) {
        return Some(s.to_string());
    }
    let t = item["team_slug"].as_str().unwrap_or("");
    let stripped = s.strip_prefix(&format!("{t}-"))?;
    catalog.contains_key(stripped).then(|| stripped.to_string())
}

fn is_instant(slug: &str, game: &serde_json::Value) -> bool {
    let v = active_version(game);
    let modes = game["modes"].as_array().map(|m| m.iter().filter(|x| x["version"].as_i64() == v).count()).unwrap_or(0);
    INSTANT_WORDS.iter().any(|w| slug.contains(w)) || modes > 40
}

fn active_version(game: &serde_json::Value) -> Option<i64> {
    game["activeVersion"].as_i64().or_else(|| game["modes"].as_array()?.iter().filter_map(|m| m["version"].as_i64()).max())
}

pub fn collect(db: &Db, paths: &Paths, s: &AppSettings, progress: &dyn Fn(&str, f64), cancel: &AtomicBool) -> anyhow::Result<CollectReport> {
    if !s.collector.enabled {
        anyhow::bail!("Collecte automatique désactivée. {ROBOTS_NOTICE}");
    }
    let client = Client::new(s);
    let src = db.source_id("api", "StakeCruncher (tracker-api)", Some(API), Some(ROBOTS_NOTICE))?;
    let coll = db.start_collection(src, Some(&crate::today()))?;
    let mut rep = CollectReport { collection_id: coll, ..Default::default() };
    let result: anyhow::Result<()> = (|| {
        // 1. Classement (pages de 100).
        progress("Classement des jeux", 0.02);
        let mut items: Vec<serde_json::Value> = Vec::new();
        let depth = s.collector.ranking_depth.clamp(100, 1_000);
        for off in (0..depth).step_by(100) {
            let url = format!("{API}/games?sort=turnover_30d&limit={}&offset={off}", (depth - off).min(100));
            let r = client.get(&url, cancel)?;
            let v: serde_json::Value = serde_json::from_slice(&r.body)?;
            let Some(page) = v["items"].as_array() else {
                anyhow::bail!("structure inattendue : pas de champ « items » dans le classement");
            };
            items.extend(page.iter().cloned());
            if page.len() < 100 {
                break;
            }
        }
        rep.games_ranked = items.len();
        if let Some(first) = items.first() {
            let sig = signature(first);
            rep.missing_fields = missing_fields(first);
            let prev: Option<String> = db
                .conn()
                .query_row("SELECT schema_signature FROM collections WHERE source_id = ?1 AND schema_signature IS NOT NULL AND id <> ?2 ORDER BY id DESC LIMIT 1", params![src, coll], |r| r.get(0))
                .optional()?;
            if prev.as_ref().is_some_and(|p| p != &sig) {
                rep.schema_changed = true;
                db.alert("WARN", "collecte", "La structure de l'API StakeCruncher a changé depuis la dernière collecte : vérifier les champs lus.");
            }
            if !rep.missing_fields.is_empty() {
                db.alert("WARN", "collecte", &format!("Champs absents de l'API (enregistrés « non disponible ») : {}", rep.missing_fields.join(", ")));
            }
            db.conn().execute("UPDATE collections SET schema_signature = ?2 WHERE id = ?1", params![coll, sig])?;
        }

        // 2. Rangs aux bets (30 j) et au turnover, calculés sur l'échantillon lu.
        let mut by_bets: Vec<usize> = (0..items.len()).filter(|&i| num(&items[i], "bet_count_past_30d").is_some()).collect();
        by_bets.sort_by(|&a, &b| num(&items[b], "bet_count_past_30d").partial_cmp(&num(&items[a], "bet_count_past_30d")).unwrap_or(std::cmp::Ordering::Equal));
        let mut by_turn: Vec<usize> = (0..items.len()).collect();
        by_turn.sort_by(|&a, &b| num(&items[b], "turnover_past_30d").partial_cmp(&num(&items[a], "turnover_past_30d")).unwrap_or(std::cmp::Ordering::Equal));
        let rank_b: std::collections::HashMap<usize, i64> = by_bets.iter().enumerate().map(|(r, &i)| (i, r as i64 + 1)).collect();
        let rank_t: std::collections::HashMap<usize, i64> = by_turn.iter().enumerate().map(|(r, &i)| (i, r as i64 + 1)).collect();
        let total_turn: f64 = items.iter().filter_map(|i| num(i, "turnover_past_30d")).sum();

        // 3. Catalogue des modes.
        progress("Catalogue des modes", 0.10);
        let cat_resp = client.get(&format!("{API}/verifier/catalog"), cancel)?;
        let cat_v: serde_json::Value = serde_json::from_slice(&cat_resp.body)?;
        let catalog: std::collections::BTreeMap<String, serde_json::Value> = cat_v["games"]
            .as_array()
            .map(|a| a.iter().filter_map(|g| Some((g["slug"].as_str()?.to_string(), g.clone()))).collect())
            .unwrap_or_default();

        // 4. Popularité de chaque jeu lu.
        let mut slot_ids: Vec<Option<i64>> = Vec::with_capacity(items.len());
        for (i, it) in items.iter().enumerate() {
            let slug = it["slug"].as_str().unwrap_or("").to_string();
            if slug.is_empty() {
                slot_ids.push(None);
                continue;
            }
            let slot_id = upsert_slot(db, &slug, it["name"].as_str().unwrap_or(&slug), it["team_name"].as_str().unwrap_or("Inconnu"), it["stake_com_slug"].as_str(), false)?;
            let players = (it["current_players_com"].as_i64(), it["peak_players_com"].as_i64());
            let rtp = num(it, "rtp").map(|v| v / 100.0);
            insert_popularity(db, slot_id, coll, "30d", num(it, "bet_count_past_30d"), num(it, "turnover_past_30d"), num(it, "turnover_past_30d").map(|t| t / total_turn.max(1.0)), rank_b.get(&i).copied(), rank_t.get(&i).copied(), players, rtp)?;
            insert_popularity(db, slot_id, coll, "7d", num(it, "bet_count_past_week"), num(it, "turnover_past_week"), None, None, None, (None, None), None)?;
            insert_popularity(db, slot_id, coll, "90d", num(it, "bet_count_rolling_90d"), num(it, "turnover_rolling_90d"), None, None, None, (None, None), None)?;
            insert_popularity(db, slot_id, coll, "yesterday", num(it, "bet_count_yesterday"), num(it, "turnover_yesterday"), None, None, None, (None, None), None)?;
            insert_popularity(db, slot_id, coll, "all", num(it, "bet_count_all_time"), num(it, "turnover_all_time"), None, None, None, (None, None), None)?;
            slot_ids.push(Some(slot_id));
        }

        // 5. Les N slots les plus jouées AU NOMBRE DE BETS (jeux instantanés et burst exclus).
        let mut chosen = Vec::new();
        for &i in by_bets.iter() {
            if chosen.len() >= s.collector.top_n {
                break;
            }
            let it = &items[i];
            let Some(cs) = catalog_slug(it, &catalog) else { continue };
            if is_instant(&cs, &catalog[&cs]) {
                continue;
            }
            chosen.push((i, cs));
        }
        rep.slots_selected = chosen.len();
        if !s.collector.fetch_luts {
            return Ok(());
        }

        // 6. Lookup tables des modes actifs, analysées exactement.
        let total = chosen.len().max(1) as f64;
        for (k, (i, cs)) in chosen.iter().enumerate() {
            let game = &catalog[cs];
            let Some(ver) = active_version(game) else { continue };
            let Some(slot_id) = slot_ids[*i] else { continue };
            let modes: Vec<&serde_json::Value> = game["modes"].as_array().map(|m| m.iter().filter(|x| x["version"].as_i64() == Some(ver) && x["active"].as_bool().unwrap_or(true)).collect()).unwrap_or_default();
            for m in modes {
                let Some(mname) = m["name"].as_str() else { continue };
                progress(&format!("Table {} / {}", items[*i]["name"].as_str().unwrap_or(cs), mname), 0.15 + 0.85 * k as f64 / total);
                let dest = lut_path(paths, cs, &ver.to_string(), mname);
                let cached = dest.exists();
                let (text, cost) = if cached {
                    let mut t = String::new();
                    flate2::read::GzDecoder::new(std::fs::File::open(&dest)?).read_to_string(&mut t)?;
                    let cost: Option<f64> = db
                        .conn()
                        .query_row("SELECT cost FROM modes WHERE slot_id = ?1 AND version = ?2 AND name = ?3", params![slot_id, ver.to_string(), mname], |r| r.get(0))
                        .optional()?;
                    (t, cost.unwrap_or(1.0))
                } else {
                    let url = format!("{API}/verifier/lookup-table?slug={cs}&version={ver}&mode={mname}&_lv=5");
                    let r = match client.get(&url, cancel) {
                        Ok(r) if r.status == 200 => r,
                        Ok(r) => {
                            rep.errors.push(format!("{cs}/{mname} : HTTP {}", r.status));
                            continue;
                        }
                        Err(e) => {
                            rep.errors.push(format!("{cs}/{mname} : {e}"));
                            if e.to_string().contains("429") || cancel.load(Ordering::Relaxed) {
                                return Err(e);
                            }
                            continue;
                        }
                    };
                    let cost = header(&r, "x-lookup-cost-multiplier").and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.0);
                    let raw = if header(&r, "x-lookup-encoding").is_some_and(|v| v.eq_ignore_ascii_case("gzip")) {
                        let mut out = Vec::new();
                        flate2::read::GzDecoder::new(&r.body[..]).read_to_end(&mut out)?;
                        out
                    } else {
                        r.body
                    };
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(dir) = dest.parent() {
                        std::fs::create_dir_all(dir)?;
                    }
                    use std::io::Write;
                    let mut gz = flate2::write::GzEncoder::new(std::fs::File::create(&dest)?, flate2::Compression::default());
                    gz.write_all(text.as_bytes())?;
                    gz.finish()?;
                    (text, cost)
                };
                match Distribution::from_lut_csv(mname, cost, None, &text) {
                    Ok((d, info)) => {
                        let source = format!("lookup table publiée {cs} v{ver} {mname} via StakeCruncher ({} lignes)", info.rows);
                        store_exact_mode(db, s, coll, slot_id, &ver.to_string(), mname, &d, &source, Some(&dest.display().to_string()))?;
                        if cached {
                            rep.luts_cached += 1;
                        } else {
                            rep.luts_downloaded += 1;
                        }
                    }
                    Err(e) => rep.errors.push(format!("{cs}/{mname} : table illisible ({e})")),
                }
            }
        }
        Ok(())
    })();
    let (status, msg) = match &result {
        Ok(()) => ("OK", format!("{} jeux classés, {} slots retenues, {} tables téléchargées, {} en cache, {} erreur(s)", rep.games_ranked, rep.slots_selected, rep.luts_downloaded, rep.luts_cached, rep.errors.len())),
        Err(e) => ("ÉCHEC", format!("Collecte interrompue : {e}")),
    };
    rep.message = msg.clone();
    db.finish_collection(coll, status, (rep.luts_downloaded + rep.luts_cached) as i64, rep.errors.len() as i64, &msg, None)?;
    db.log("collecte", &msg);
    if let Err(e) = result {
        db.alert("ERROR", "collecte", &msg);
        return Err(e);
    }
    progress("Terminé", 1.0);
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default() {
        let db = Db::open_in_memory().unwrap();
        let paths = Paths::at(&std::env::temp_dir());
        let err = collect(&db, &paths, &AppSettings::default(), &|_, _| {}, &AtomicBool::new(false)).unwrap_err();
        assert!(err.to_string().contains("désactivée"));
    }

    #[test]
    fn schema_helpers() {
        let item = serde_json::json!({"slug": "valkyrie-x", "team_slug": "valkyrie", "name": "X", "bet_count_past_30d": 5});
        let mut cat = std::collections::BTreeMap::new();
        cat.insert("x".to_string(), serde_json::json!({"slug": "x", "modes": []}));
        assert_eq!(catalog_slug(&item, &cat).as_deref(), Some("x"));
        assert!(missing_fields(&item).contains(&"turnover_past_30d".to_string()));
        assert_eq!(signature(&item), "bet_count_past_30d,name,slug,team_slug");
    }
}
