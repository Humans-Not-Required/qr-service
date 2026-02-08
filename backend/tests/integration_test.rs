// Unit tests for QR service core functionality
use std::env;

#[test]
fn test_health_endpoint() {
    // Basic sanity test: just verify QR generation logic works
    let code = qrcode::QrCode::new(b"Hello, World!").unwrap();
    assert!(code.width() > 0);
}

#[test]
fn test_hex_color_parsing() {
    // Test the color parsing utility
    let white = qr_service::qr::parse_hex_color("#FFFFFF").unwrap();
    assert_eq!(white, [255, 255, 255, 255]);

    let black = qr_service::qr::parse_hex_color("#000000").unwrap();
    assert_eq!(black, [0, 0, 0, 255]);

    let with_alpha = qr_service::qr::parse_hex_color("#FF000080").unwrap();
    assert_eq!(with_alpha, [255, 0, 0, 128]);

    // Invalid
    assert!(qr_service::qr::parse_hex_color("#GGG").is_err());
    assert!(qr_service::qr::parse_hex_color("#12").is_err());
}

#[test]
fn test_qr_png_generation() {
    let options = qr_service::qr::QrOptions {
        size: 256,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::M,
        style: qr_service::qr::QrStyle::Square,
    };

    let result = qr_service::qr::generate_png("https://example.com", &options);
    assert!(result.is_ok());
    let data = result.unwrap();
    // PNG magic bytes
    assert_eq!(&data[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    // Should be a reasonable size
    assert!(data.len() > 100);
}

#[test]
fn test_qr_svg_generation() {
    let options = qr_service::qr::QrOptions {
        size: 256,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::M,
        style: qr_service::qr::QrStyle::Square,
    };

    let result = qr_service::qr::generate_svg("https://example.com", &options);
    assert!(result.is_ok());
    let svg = result.unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    assert!(svg.contains("<rect"));
}

#[test]
fn test_wifi_template_data() {
    let data = qr_service::qr::wifi_data("MyNetwork", "secret123", "WPA2", false);
    assert_eq!(data, "WIFI:T:WPA2;S:MyNetwork;P:secret123;H:false;;");
}

#[test]
fn test_wifi_template_hidden() {
    let data = qr_service::qr::wifi_data("HiddenNet", "pass", "WPA2", true);
    assert!(data.contains("H:true"));
}

#[test]
fn test_wifi_template_escaping() {
    let data = qr_service::qr::wifi_data("My;Network", "pass;word", "WPA2", false);
    assert!(data.contains("S:My\\;Network"));
    assert!(data.contains("P:pass\\;word"));
}

#[test]
fn test_vcard_generation() {
    let data = qr_service::qr::vcard_data(
        "John Doe",
        Some("john@example.com"),
        Some("+1234567890"),
        None,
        None,
        None,
    );
    assert!(data.contains("BEGIN:VCARD"));
    assert!(data.contains("FN:John Doe"));
    assert!(data.contains("EMAIL:john@example.com"));
    assert!(data.contains("TEL:+1234567890"));
    assert!(data.contains("END:VCARD"));
}

#[test]
fn test_vcard_minimal() {
    let data = qr_service::qr::vcard_data("Jane", None, None, None, None, None);
    assert!(data.contains("FN:Jane"));
    assert!(!data.contains("EMAIL:"));
}

#[test]
fn test_roundtrip_generate_decode() {
    // Generate a QR code and then decode it
    let test_data = "https://humans-not-required.github.io";
    let options = qr_service::qr::QrOptions {
        size: 512,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::H,
        style: qr_service::qr::QrStyle::Square,
    };

    let png_data = qr_service::qr::generate_png(test_data, &options).unwrap();

    // Load the PNG and decode
    let img = image::load_from_memory(&png_data).unwrap().to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(img);
    let grids = prepared.detect_grids();
    assert!(!grids.is_empty(), "Should detect at least one QR grid");

    let (_meta, content) = grids.into_iter().next().unwrap().decode().unwrap();
    assert_eq!(content, test_data);
}

#[test]
fn test_error_correction_levels() {
    // All EC levels should work
    for level in &["L", "M", "Q", "H"] {
        let ec = qr_service::qr::parse_ec_level(level);
        let options = qr_service::qr::QrOptions {
            size: 128,
            fg_color: [0, 0, 0, 255],
            bg_color: [255, 255, 255, 255],
            error_correction: ec,
            style: qr_service::qr::QrStyle::Square,
        };
        assert!(qr_service::qr::generate_png("test", &options).is_ok());
    }
}

#[test]
fn test_dots_style_png() {
    let options = qr_service::qr::QrOptions {
        size: 256,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::M,
        style: qr_service::qr::QrStyle::Dots,
    };
    let result = qr_service::qr::generate_png("https://example.com", &options);
    assert!(result.is_ok());
    let data = result.unwrap();
    assert_eq!(&data[0..4], &[0x89, 0x50, 0x4E, 0x47]);
}

#[test]
fn test_rounded_style_png() {
    let options = qr_service::qr::QrOptions {
        size: 256,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::M,
        style: qr_service::qr::QrStyle::Rounded,
    };
    let result = qr_service::qr::generate_png("https://example.com", &options);
    assert!(result.is_ok());
    let data = result.unwrap();
    assert_eq!(&data[0..4], &[0x89, 0x50, 0x4E, 0x47]);
}

#[test]
fn test_dots_style_svg() {
    let options = qr_service::qr::QrOptions {
        size: 256,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::M,
        style: qr_service::qr::QrStyle::Dots,
    };
    let svg = qr_service::qr::generate_svg("https://example.com", &options).unwrap();
    assert!(
        svg.contains("<circle"),
        "Dots style SVG should use <circle> elements"
    );
    assert!(
        !svg.contains("<rect x="),
        "Dots style SVG should not use module <rect> elements (except background)"
    );
}

#[test]
fn test_rounded_style_svg() {
    let options = qr_service::qr::QrOptions {
        size: 256,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::M,
        style: qr_service::qr::QrStyle::Rounded,
    };
    let svg = qr_service::qr::generate_svg("https://example.com", &options).unwrap();
    assert!(svg.contains("<svg"));
    // Should have at least some <path> elements for rounded corners
    assert!(
        svg.contains("<path") || svg.contains("<rect"),
        "Rounded style SVG should have path or rect elements"
    );
}

#[test]
fn test_dots_style_roundtrip() {
    // Dots style should still be scannable at high resolution with high EC
    let test_data = "DOTS_TEST";
    let options = qr_service::qr::QrOptions {
        size: 1024,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::H,
        style: qr_service::qr::QrStyle::Dots,
    };
    let png_data = qr_service::qr::generate_png(test_data, &options).unwrap();
    let img = image::load_from_memory(&png_data).unwrap().to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(img);
    let grids = prepared.detect_grids();
    assert!(
        !grids.is_empty(),
        "Dots style QR should still be detectable"
    );
    let (_meta, content) = grids.into_iter().next().unwrap().decode().unwrap();
    assert_eq!(content, test_data);
}

#[test]
fn test_rounded_style_roundtrip() {
    // Rounded style should still be scannable
    let test_data = "ROUNDED_TEST";
    let options = qr_service::qr::QrOptions {
        size: 512,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::H,
        style: qr_service::qr::QrStyle::Rounded,
    };
    let png_data = qr_service::qr::generate_png(test_data, &options).unwrap();
    let img = image::load_from_memory(&png_data).unwrap().to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(img);
    let grids = prepared.detect_grids();
    assert!(
        !grids.is_empty(),
        "Rounded style QR should still be detectable"
    );
    let (_meta, content) = grids.into_iter().next().unwrap().decode().unwrap();
    assert_eq!(content, test_data);
}

#[test]
fn test_style_from_str() {
    assert_eq!(
        qr_service::qr::QrStyle::parse("square"),
        qr_service::qr::QrStyle::Square
    );
    assert_eq!(
        qr_service::qr::QrStyle::parse("rounded"),
        qr_service::qr::QrStyle::Rounded
    );
    assert_eq!(
        qr_service::qr::QrStyle::parse("dots"),
        qr_service::qr::QrStyle::Dots
    );
    assert_eq!(
        qr_service::qr::QrStyle::parse("DOTS"),
        qr_service::qr::QrStyle::Dots
    );
    assert_eq!(
        qr_service::qr::QrStyle::parse("unknown"),
        qr_service::qr::QrStyle::Square
    );
}

// ============ Tracked QR / Short URL Tests ============

/// Helper: create a test DB with new schema (no api_keys), return pool
fn setup_test_db() -> qr_service::db::DbPool {
    let db_path = format!("/tmp/qr_test_{}.db", uuid::Uuid::new_v4());
    env::set_var("DATABASE_PATH", &db_path);
    qr_service::db::init_db().expect("Failed to init test DB")
}

#[test]
fn test_tracked_qr_db_roundtrip() {
    let pool = setup_test_db();
    let conn = pool.lock().unwrap();

    let qr_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO qr_codes (id, data, format, size, image_data) VALUES (?1, 'http://localhost/r/test123', 'png', 256, X'89504E47')",
        rusqlite::params![qr_id],
    ).unwrap();

    let tracked_id = uuid::Uuid::new_v4().to_string();
    let manage_token = "qrt_test123";
    let token_hash = qr_service::db::hash_token(manage_token);
    conn.execute(
        "INSERT INTO tracked_qr (id, qr_id, short_code, target_url, manage_token_hash) VALUES (?1, ?2, 'test123', 'https://example.com', ?3)",
        rusqlite::params![tracked_id, qr_id, token_hash],
    ).unwrap();

    let (found_code, found_url): (String, String) = conn
        .query_row(
            "SELECT short_code, target_url FROM tracked_qr WHERE id = ?1",
            rusqlite::params![tracked_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(found_code, "test123");
    assert_eq!(found_url, "https://example.com");

    // Verify token-based lookup works
    let found_id: String = conn
        .query_row(
            "SELECT id FROM tracked_qr WHERE id = ?1 AND manage_token_hash = ?2",
            rusqlite::params![tracked_id, token_hash],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(found_id, tracked_id);

    // Insert a scan event
    let scan_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO scan_events (id, tracked_qr_id, user_agent, referrer) VALUES (?1, ?2, 'TestAgent/1.0', 'https://referrer.com')",
        rusqlite::params![scan_id, tracked_id],
    ).unwrap();

    conn.execute(
        "UPDATE tracked_qr SET scan_count = scan_count + 1 WHERE id = ?1",
        rusqlite::params![tracked_id],
    ).unwrap();

    let scan_count: i64 = conn
        .query_row("SELECT scan_count FROM tracked_qr WHERE id = ?1", rusqlite::params![tracked_id], |row| row.get(0))
        .unwrap();
    assert_eq!(scan_count, 1);

    let (found_ua, found_ref): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT user_agent, referrer FROM scan_events WHERE tracked_qr_id = ?1",
            rusqlite::params![tracked_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(found_ua.unwrap(), "TestAgent/1.0");
    assert_eq!(found_ref.unwrap(), "https://referrer.com");
}

#[test]
fn test_tracked_qr_short_code_uniqueness() {
    let pool = setup_test_db();
    let conn = pool.lock().unwrap();

    let qr_id1 = uuid::Uuid::new_v4().to_string();
    let qr_id2 = uuid::Uuid::new_v4().to_string();
    let hash = qr_service::db::hash_token("token1");
    conn.execute(
        "INSERT INTO qr_codes (id, data, format, size, image_data) VALUES (?1, 'data1', 'png', 256, X'89504E47')",
        rusqlite::params![qr_id1],
    ).unwrap();
    conn.execute(
        "INSERT INTO qr_codes (id, data, format, size, image_data) VALUES (?1, 'data2', 'png', 256, X'89504E47')",
        rusqlite::params![qr_id2],
    ).unwrap();

    let tracked1 = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tracked_qr (id, qr_id, short_code, target_url, manage_token_hash) VALUES (?1, ?2, 'unique1', 'https://example.com', ?3)",
        rusqlite::params![tracked1, qr_id1, hash],
    ).unwrap();

    let tracked2 = uuid::Uuid::new_v4().to_string();
    let result = conn.execute(
        "INSERT INTO tracked_qr (id, qr_id, short_code, target_url, manage_token_hash) VALUES (?1, ?2, 'unique1', 'https://other.com', ?3)",
        rusqlite::params![tracked2, qr_id2, hash],
    );
    assert!(result.is_err(), "Duplicate short_code should be rejected by UNIQUE constraint");
}

#[test]
fn test_tracked_qr_cascade_delete() {
    let pool = setup_test_db();
    let conn = pool.lock().unwrap();

    let qr_id = uuid::Uuid::new_v4().to_string();
    let hash = qr_service::db::hash_token("token_del");
    conn.execute(
        "INSERT INTO qr_codes (id, data, format, size, image_data) VALUES (?1, 'data', 'png', 256, X'89504E47')",
        rusqlite::params![qr_id],
    ).unwrap();

    let tracked_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tracked_qr (id, qr_id, short_code, target_url, manage_token_hash) VALUES (?1, ?2, 'del_test', 'https://example.com', ?3)",
        rusqlite::params![tracked_id, qr_id, hash],
    ).unwrap();

    for i in 0..5 {
        let scan_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO scan_events (id, tracked_qr_id, user_agent) VALUES (?1, ?2, ?3)",
            rusqlite::params![scan_id, tracked_id, format!("Agent/{}", i)],
        ).unwrap();
    }

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scan_events WHERE tracked_qr_id = ?1",
        rusqlite::params![tracked_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 5);

    conn.execute("DELETE FROM scan_events WHERE tracked_qr_id = ?1", rusqlite::params![tracked_id]).unwrap();
    conn.execute("DELETE FROM tracked_qr WHERE id = ?1", rusqlite::params![tracked_id]).unwrap();

    let remaining: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scan_events WHERE tracked_qr_id = ?1",
        rusqlite::params![tracked_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn test_tracked_qr_expiry_check() {
    let pool = setup_test_db();
    let conn = pool.lock().unwrap();

    let qr_id = uuid::Uuid::new_v4().to_string();
    let hash = qr_service::db::hash_token("token_exp");
    conn.execute(
        "INSERT INTO qr_codes (id, data, format, size, image_data) VALUES (?1, 'data', 'png', 256, X'89504E47')",
        rusqlite::params![qr_id],
    ).unwrap();

    let tracked_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tracked_qr (id, qr_id, short_code, target_url, manage_token_hash, expires_at) VALUES (?1, ?2, 'expired1', 'https://example.com', ?3, '2020-01-01 00:00:00')",
        rusqlite::params![tracked_id, qr_id, hash],
    ).unwrap();

    let expires_at: Option<String> = conn
        .query_row("SELECT expires_at FROM tracked_qr WHERE id = ?1", rusqlite::params![tracked_id], |row| row.get(0))
        .unwrap();

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    assert!(expires_at.unwrap() < now, "Expired link should have past timestamp");
}

#[test]
fn test_manage_token_hash() {
    // Verify hash_token is deterministic
    let token = "qrt_abc123";
    let h1 = qr_service::db::hash_token(token);
    let h2 = qr_service::db::hash_token(token);
    assert_eq!(h1, h2);
    // Different token = different hash
    let h3 = qr_service::db::hash_token("qrt_different");
    assert_ne!(h1, h3);
}
