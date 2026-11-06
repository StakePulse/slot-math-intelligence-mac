//! Base SQLite locale. Schéma versionné par `PRAGMA user_version` : chaque migration s'ajoute à la
//! liste sans toucher aux précédentes (base évolutive, règle n°10).
//!
//! Traçabilité (règle n°4) : toute donnée de marché pointe vers une `collection` (source, date,
//! statut) ; toute statistique porte sa nature (EXACT / IMPORTÉ / MONTE CARLO).

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub struct Db {
    conn: Mutex<Connection>,
}

const MIGRATIONS: &[&str] = &[
    // v1 — schéma initial
    r#"
    CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS activity (id INTEGER PRIMARY KEY, at TEXT NOT NULL, kind TEXT NOT NULL, message TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS alerts (id INTEGER PRIMARY KEY, created_at TEXT NOT NULL, level TEXT NOT NULL, scope TEXT NOT NULL, message TEXT NOT NULL, resolved INTEGER NOT NULL DEFAULT 0);

    CREATE TABLE IF NOT EXISTS sources (id INTEGER PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL UNIQUE, url TEXT, note TEXT);
    CREATE TABLE IF NOT EXISTS collections (
        id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL REFERENCES sources(id),
        started_at TEXT NOT NULL, finished_at TEXT, data_date TEXT, status TEXT NOT NULL,
        items INTEGER NOT NULL DEFAULT 0, errors INTEGER NOT NULL DEFAULT 0, message TEXT, schema_signature TEXT);

    CREATE TABLE IF NOT EXISTS providers (id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS slots (
        id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL, provider_id INTEGER REFERENCES providers(id),
        stake_slug TEXT, ours INTEGER NOT NULL DEFAULT 0, first_seen TEXT NOT NULL, last_seen TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS popularity (
        id INTEGER PRIMARY KEY, slot_id INTEGER NOT NULL REFERENCES slots(id), collection_id INTEGER NOT NULL REFERENCES collections(id),
        period TEXT NOT NULL, bets REAL, turnover REAL, turnover_share REAL, rank_bets INTEGER, rank_turnover INTEGER,
        players_current INTEGER, players_peak INTEGER, rtp_displayed REAL,
        UNIQUE(slot_id, collection_id, period));
    CREATE TABLE IF NOT EXISTS modes (
        id INTEGER PRIMARY KEY, slot_id INTEGER NOT NULL REFERENCES slots(id), version TEXT NOT NULL, name TEXT NOT NULL,
        cost REAL NOT NULL, kind TEXT NOT NULL, UNIQUE(slot_id, version, name));
    CREATE TABLE IF NOT EXISTS mode_stats (
        id INTEGER PRIMARY KEY, mode_id INTEGER NOT NULL REFERENCES modes(id), collection_id INTEGER NOT NULL REFERENCES collections(id),
        nature TEXT NOT NULL, source TEXT NOT NULL,
        rtp REAL, hit REAL, sd_per_price REAL, max_mult REAL, p_max REAL, median REAL, q25 REAL, q75 REAL, q90 REAL,
        share_100x REAL, top1 REAL, p_1000x REAL, vol_class TEXT, vol_score REAL,
        figures TEXT NOT NULL, lut_path TEXT, lut_fingerprint TEXT,
        UNIQUE(mode_id, collection_id));

    CREATE TABLE IF NOT EXISTS models (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
        current_version INTEGER NOT NULL, status TEXT NOT NULL, sources TEXT NOT NULL DEFAULT '[]');
    CREATE TABLE IF NOT EXISTS model_versions (
        id INTEGER PRIMARY KEY, model_id TEXT NOT NULL REFERENCES models(id), version INTEGER NOT NULL, created_at TEXT NOT NULL,
        spec TEXT NOT NULL, params TEXT, notes TEXT NOT NULL DEFAULT '[]', distribution TEXT NOT NULL, fingerprint TEXT NOT NULL,
        change_note TEXT NOT NULL DEFAULT '', status TEXT NOT NULL,
        UNIQUE(model_id, version));
    CREATE TABLE IF NOT EXISTS simulations (
        id INTEGER PRIMARY KEY, created_at TEXT NOT NULL, target_kind TEXT NOT NULL, model_id TEXT, version INTEGER,
        label TEXT NOT NULL, config TEXT NOT NULL, result TEXT NOT NULL, spins INTEGER NOT NULL, rtp REAL, completed INTEGER NOT NULL,
        elapsed_ms INTEGER NOT NULL, fingerprint TEXT);
    CREATE TABLE IF NOT EXISTS validations (
        id INTEGER PRIMARY KEY, model_id TEXT NOT NULL, version INTEGER NOT NULL, created_at TEXT NOT NULL, status TEXT NOT NULL, report TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS prompts (
        id INTEGER PRIMARY KEY, model_id TEXT NOT NULL, version INTEGER NOT NULL, created_at TEXT NOT NULL, markdown TEXT NOT NULL, path TEXT);

    CREATE TABLE IF NOT EXISTS documents (
        id INTEGER PRIMARY KEY, sha256 TEXT NOT NULL UNIQUE, filename TEXT NOT NULL, kind TEXT NOT NULL, category TEXT NOT NULL,
        path TEXT NOT NULL, size INTEGER NOT NULL, imported_at TEXT NOT NULL, pages INTEGER, text_status TEXT NOT NULL,
        text TEXT, note TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT '', is_lut INTEGER NOT NULL DEFAULT 0);
    CREATE TABLE IF NOT EXISTS extracted_facts (
        id INTEGER PRIMARY KEY, document_id INTEGER NOT NULL REFERENCES documents(id), key TEXT NOT NULL, label TEXT NOT NULL,
        value TEXT NOT NULL, value_num REAL, page INTEGER, context TEXT NOT NULL, status TEXT NOT NULL,
        validated_at TEXT, validated_value TEXT);

    CREATE INDEX IF NOT EXISTS idx_pop_slot ON popularity(slot_id, collection_id);
    CREATE INDEX IF NOT EXISTS idx_stats_mode ON mode_stats(mode_id, collection_id);
    CREATE INDEX IF NOT EXISTS idx_sims_model ON simulations(model_id, version);
    CREATE INDEX IF NOT EXISTS idx_facts_doc ON extracted_facts(document_id);
    "#,
];

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(10))?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        let c = self.conn();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(v as usize) {
            c.execute_batch(&format!("BEGIN; {sql} PRAGMA user_version = {}; COMMIT;", i + 1))?;
        }
        Ok(())
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn schema_version(&self) -> i64 {
        self.conn().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0)
    }

    pub fn log(&self, kind: &str, message: &str) {
        let _ = self.conn().execute("INSERT INTO activity(at, kind, message) VALUES (?1, ?2, ?3)", params![crate::now(), kind, message]);
    }

    pub fn alert(&self, level: &str, scope: &str, message: &str) {
        let c = self.conn();
        // Pas de doublon d'une alerte encore ouverte.
        let exists: Option<i64> = c
            .query_row("SELECT id FROM alerts WHERE resolved = 0 AND scope = ?1 AND message = ?2", params![scope, message], |r| r.get(0))
            .optional()
            .unwrap_or(None);
        if exists.is_none() {
            let _ = c.execute("INSERT INTO alerts(created_at, level, scope, message) VALUES (?1, ?2, ?3, ?4)", params![crate::now(), level, scope, message]);
        }
    }

    pub fn setting_get(&self, key: &str) -> Option<String> {
        self.conn().query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn setting_set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.conn().execute("INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
        Ok(())
    }

    pub fn source_id(&self, kind: &str, name: &str, url: Option<&str>, note: Option<&str>) -> anyhow::Result<i64> {
        let c = self.conn();
        c.execute("INSERT OR IGNORE INTO sources(kind, name, url, note) VALUES (?1, ?2, ?3, ?4)", params![kind, name, url, note])?;
        Ok(c.query_row("SELECT id FROM sources WHERE name = ?1", params![name], |r| r.get(0))?)
    }

    pub fn start_collection(&self, source_id: i64, data_date: Option<&str>) -> anyhow::Result<i64> {
        let c = self.conn();
        c.execute("INSERT INTO collections(source_id, started_at, data_date, status) VALUES (?1, ?2, ?3, 'EN COURS')", params![source_id, crate::now(), data_date])?;
        Ok(c.last_insert_rowid())
    }

    pub fn finish_collection(&self, id: i64, status: &str, items: i64, errors: i64, message: &str, signature: Option<&str>) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE collections SET finished_at = ?2, status = ?3, items = ?4, errors = ?5, message = ?6, schema_signature = ?7 WHERE id = ?1",
            params![id, crate::now(), status, items, errors, message, signature],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.schema_version(), MIGRATIONS.len() as i64);
        db.migrate().unwrap();
        db.alert("WARN", "test", "message");
        db.alert("WARN", "test", "message");
        let n: i64 = db.conn().query_row("SELECT COUNT(*) FROM alerts", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }
}
