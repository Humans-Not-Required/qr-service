use rusqlite::{Connection, Result};
use std::sync::Mutex;

pub type DbPool = Mutex<Connection>;

pub fn init_db() -> Result<DbPool> {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "qr_service.db".to_string());
    let conn = Connection::open(&db_path)?;

    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;
        
        CREATE TABLE IF NOT EXISTS qr_codes (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL,
            format TEXT NOT NULL DEFAULT 'png',
            size INTEGER NOT NULL DEFAULT 256,
            fg_color TEXT NOT NULL DEFAULT '#000000',
            bg_color TEXT NOT NULL DEFAULT '#FFFFFF',
            error_correction TEXT NOT NULL DEFAULT 'M',
            style TEXT NOT NULL DEFAULT 'square',
            template TEXT,
            image_data BLOB,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        
        CREATE TABLE IF NOT EXISTS tracked_qr (
            id TEXT PRIMARY KEY,
            qr_id TEXT NOT NULL,
            short_code TEXT NOT NULL UNIQUE,
            target_url TEXT NOT NULL,
            manage_token_hash TEXT NOT NULL,
            scan_count INTEGER NOT NULL DEFAULT 0,
            expires_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (qr_id) REFERENCES qr_codes(id)
        );
        
        CREATE TABLE IF NOT EXISTS scan_events (
            id TEXT PRIMARY KEY,
            tracked_qr_id TEXT NOT NULL,
            scanned_at TEXT NOT NULL DEFAULT (datetime('now')),
            user_agent TEXT,
            referrer TEXT,
            FOREIGN KEY (tracked_qr_id) REFERENCES tracked_qr(id)
        );
        
        CREATE INDEX IF NOT EXISTS idx_qr_codes_created ON qr_codes(created_at);
        CREATE INDEX IF NOT EXISTS idx_tracked_qr_short_code ON tracked_qr(short_code);
        CREATE INDEX IF NOT EXISTS idx_tracked_qr_manage_token ON tracked_qr(manage_token_hash);
    ",
    )?;

    Ok(Mutex::new(conn))
}

pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
