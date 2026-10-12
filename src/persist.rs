//! Local persistence: a tiny SQLite key-value store for shell state
//! (theme, panels, sizes, sides, tabs). Lives under the platform data dir.

use std::path::PathBuf;

pub struct Store {
    conn: rusqlite::Connection,
}

impl Store {
    pub fn open_default() -> Self {
        let dir = dirs::data_dir()
            .map(|d| d.join("prose"))
            .unwrap_or_else(|| PathBuf::from("."));
        let _ = std::fs::create_dir_all(&dir);
        let conn = rusqlite::Connection::open(dir.join("prose.db")).expect("open prose db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )
        .expect("init prose db");
        Self { conn }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
            .ok()
    }

    pub fn set(&self, key: &str, value: &str) {
        let _ = self.conn.execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        );
    }
}
