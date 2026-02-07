use rusqlite::{Connection, Result};
use std::sync::Mutex;

pub type DbPool = Mutex<Connection>;

pub fn init_db() -> Result<DbPool> {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "qr_service.db".to_string());
    let conn = Connection::open(&db_path)?;
    
    conn.execute_batch("
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;
        
        CREATE TABLE IF NOT EXISTS api_keys (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_used_at TEXT,
            requests_count INTEGER NOT NULL DEFAULT 0,
            rate_limit INTEGER NOT NULL DEFAULT 100,
            is_admin INTEGER NOT NULL DEFAULT 0,
            active INTEGER NOT NULL DEFAULT 1
        );
        
        CREATE TABLE IF NOT EXISTS qr_codes (
            id TEXT PRIMARY KEY,
            api_key_id TEXT NOT NULL,
            data TEXT NOT NULL,
            format TEXT NOT NULL DEFAULT 'png',
            size INTEGER NOT NULL DEFAULT 256,
            fg_color TEXT NOT NULL DEFAULT '#000000',
            bg_color TEXT NOT NULL DEFAULT '#FFFFFF',
            error_correction TEXT NOT NULL DEFAULT 'M',
            style TEXT NOT NULL DEFAULT 'square',
            template TEXT,
            image_data BLOB,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (api_key_id) REFERENCES api_keys(id)
        );
        
        CREATE TABLE IF NOT EXISTS tracked_qr (
            id TEXT PRIMARY KEY,
            qr_id TEXT NOT NULL,
            short_code TEXT NOT NULL UNIQUE,
            target_url TEXT NOT NULL,
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
        
        CREATE INDEX IF NOT EXISTS idx_qr_codes_api_key ON qr_codes(api_key_id);
        CREATE INDEX IF NOT EXISTS idx_qr_codes_created ON qr_codes(created_at);
        CREATE INDEX IF NOT EXISTS idx_tracked_qr_short_code ON tracked_qr(short_code);
        CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash);
    ")?;
    
    // Create default admin key if none exists
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM api_keys WHERE is_admin = 1",
        [],
        |row| row.get(0),
    )?;
    
    if count == 0 {
        let admin_key = format!("qrs_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        let key_hash = hash_key(&admin_key);
        conn.execute(
            "INSERT INTO api_keys (id, name, key_hash, is_admin, rate_limit) VALUES (?1, ?2, ?3, 1, 10000)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                "Default Admin Key",
                key_hash,
            ],
        )?;
        println!("===========================================");
        println!("  Default admin API key created:");
        println!("  {}", admin_key);
        println!("  Save this key! It won't be shown again.");
        println!("===========================================");
    }
    
    Ok(Mutex::new(conn))
}

pub fn hash_key(key: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}
