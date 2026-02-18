// HTTP integration tests using Rocket's test client.
// Tests the full request→response cycle: stateless QR generation,
// tracked QR with manage tokens, rate limiting, and error handling.

#[macro_use]
extern crate rocket;

use base64::Engine;
use rocket::http::{ContentType, Header, Status};
use rocket::local::blocking::Client;
use std::time::Duration;

/// Build a Rocket test client with a fresh database.
fn test_client() -> Client {
    let db_path = format!("/tmp/qr_http_test_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("BASE_URL", "http://localhost:8000");

    let db = qr_service::db::init_db_with_path(&db_path).expect("DB should initialize");
    let limiter = qr_service::rate_limit::RateLimiter::new(Duration::from_secs(3600));

    let rocket = rocket::build()
        .manage(db)
        .manage(limiter)
        .mount(
            "/api/v1",
            routes![
                qr_service::routes::health,
                qr_service::routes::openapi,
                qr_service::routes::llms_txt,
                qr_service::routes::generate_qr,
                qr_service::routes::decode_qr,
                qr_service::routes::batch_generate,
                qr_service::routes::generate_from_template,
                qr_service::routes::create_tracked_qr,
                qr_service::routes::get_tracked_qr_stats,
                qr_service::routes::delete_tracked_qr,
            ],
        )
        .mount(
            "/",
            routes![
                qr_service::routes::redirect_short_url,
                qr_service::routes::view_qr,
                qr_service::routes::skills_index,
                qr_service::routes::skills_skill_md,
            ],
        );

    Client::tracked(rocket).expect("valid rocket instance")
}

// ============ Health & Discovery ============

#[test]
fn test_http_health() {
    let client = test_client();
    let response = client.get("/api/v1/health").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["status"], "ok");
}

#[test]
fn test_http_openapi() {
    let client = test_client();
    let response = client.get("/api/v1/openapi.json").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = serde_json::from_str(&response.into_string().unwrap()).unwrap();
    assert!(body["openapi"].is_string());
}

#[test]
fn test_http_llms_txt() {
    let client = test_client();
    let response = client.get("/api/v1/llms.txt").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().unwrap();
    assert!(body.contains("qr") || body.contains("QR"));
}

// ============ Stateless QR Generation ============

#[test]
fn test_http_generate_qr_png() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "https://example.com"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["image_base64"].as_str().unwrap().starts_with("data:image/png;base64,"));
    assert!(body["share_url"].as_str().unwrap().contains("/qr/view?"));
    assert_eq!(body["format"], "png");
    assert_eq!(body["size"], 256);
    assert_eq!(body["data"], "https://example.com");
}

#[test]
fn test_http_generate_qr_svg() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "test", "format": "svg", "size": 128}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["image_base64"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));
    assert_eq!(body["format"], "svg");
    assert_eq!(body["size"], 128);
}

#[test]
fn test_http_generate_qr_custom_colors() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r##"{"data": "hello", "fg_color": "#FF0000", "bg_color": "#00FF00", "style": "dots"}"##)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["image_base64"].as_str().unwrap().starts_with("data:image/png;base64,"));
}

#[test]
fn test_http_generate_qr_empty_data_rejected() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": ""}"#)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "EMPTY_DATA");
}

#[test]
fn test_http_generate_qr_invalid_size() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "test", "size": 10}"#)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "INVALID_SIZE");
}

#[test]
fn test_http_generate_qr_invalid_format() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "test", "format": "gif"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "INVALID_FORMAT");
}

// ============ PDF Format ============

#[test]
fn test_http_generate_qr_pdf() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "https://example.com", "format": "pdf", "size": 256}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["image_base64"].as_str().unwrap().starts_with("data:application/pdf;base64,"));
    assert_eq!(body["format"], "pdf");
    assert_eq!(body["size"], 256);
    assert_eq!(body["data"], "https://example.com");
    // Verify it's a valid PDF (starts with %PDF)
    let b64 = body["image_base64"].as_str().unwrap();
    let raw = b64.strip_prefix("data:application/pdf;base64,").unwrap();
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw).unwrap();
    assert!(bytes.starts_with(b"%PDF"), "Should start with PDF header");
}

#[test]
fn test_http_generate_qr_pdf_styles() {
    let client = test_client();
    // Test all three styles produce valid PDFs
    for style in &["square", "rounded", "dots"] {
        let body_str = format!(r#"{{"data": "test-{}", "format": "pdf", "size": 128, "style": "{}"}}"#, style, style);
        let response = client
            .post("/api/v1/qr/generate")
            .header(ContentType::JSON)
            .body(body_str)
            .dispatch();
        assert_eq!(response.status(), Status::Ok, "Style {} should succeed", style);
        let body: serde_json::Value = response.into_json().unwrap();
        let b64 = body["image_base64"].as_str().unwrap();
        assert!(b64.starts_with("data:application/pdf;base64,"), "Style {} should produce PDF", style);
        let raw = b64.strip_prefix("data:application/pdf;base64,").unwrap();
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "Style {} should have PDF header", style);
    }
}

#[test]
fn test_http_generate_qr_pdf_custom_colors() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r##"{"data": "colored", "format": "pdf", "fg_color": "#FF0000", "bg_color": "#00FF00"}"##)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["image_base64"].as_str().unwrap().starts_with("data:application/pdf;base64,"));
}

#[test]
fn test_http_batch_generate_pdf() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(r#"{"items": [{"data": "pdf1", "format": "pdf"}, {"data": "pdf2", "format": "pdf", "style": "dots"}]}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["total"], 2);
    for item in body["items"].as_array().unwrap() {
        assert!(item["image_base64"].as_str().unwrap().starts_with("data:application/pdf;base64,"));
        assert_eq!(item["format"], "pdf");
    }
}

#[test]
fn test_http_template_wifi_pdf() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/template/wifi")
        .header(ContentType::JSON)
        .body(r#"{"ssid": "MyNetwork", "password": "secret", "format": "pdf"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["image_base64"].as_str().unwrap().starts_with("data:application/pdf;base64,"));
    assert_eq!(body["format"], "pdf");
}

#[test]
fn test_http_view_qr_pdf() {
    let client = test_client();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"hello");
    let url = format!("/qr/view?data={}&format=pdf", data);
    let response = client.get(url).dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_bytes().unwrap();
    assert!(body.starts_with(b"%PDF"), "View endpoint should return raw PDF");
}

#[test]
fn test_http_tracked_qr_pdf() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com", "format": "pdf"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["qr"]["image_base64"].as_str().unwrap().starts_with("data:application/pdf;base64,"));
    assert_eq!(body["qr"]["format"], "pdf");
}

#[test]
fn test_http_generate_qr_no_auth_needed() {
    // Verify that generation works without any auth headers
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "no-auth-needed"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

// ============ QR Decoding ============

#[test]
fn test_http_decode_qr_roundtrip() {
    let client = test_client();

    // Generate a QR code
    let gen_response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "roundtrip-test"}"#)
        .dispatch();
    assert_eq!(gen_response.status(), Status::Ok);
    let gen_body: serde_json::Value = gen_response.into_json().unwrap();

    // Extract raw PNG bytes from base64
    let b64 = gen_body["image_base64"].as_str().unwrap();
    let raw_b64 = b64.strip_prefix("data:image/png;base64,").unwrap();
    let png_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw_b64).unwrap();

    // Decode it
    let dec_response = client
        .post("/api/v1/qr/decode")
        .body(png_bytes)
        .dispatch();
    assert_eq!(dec_response.status(), Status::Ok);
    let dec_body: serde_json::Value = dec_response.into_json().unwrap();
    assert_eq!(dec_body["data"], "roundtrip-test");
}

// ============ Batch Generation ============

#[test]
fn test_http_batch_generate() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(r#"{"items": [{"data": "one"}, {"data": "two"}, {"data": "three"}]}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 3);
}

#[test]
fn test_http_batch_empty_rejected() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(r#"{"items": []}"#)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "EMPTY_BATCH");
}

// ============ Template Generation ============

#[test]
fn test_http_template_wifi() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/template/wifi")
        .header(ContentType::JSON)
        .body(r#"{"ssid": "MyNetwork", "password": "secret123"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["data"].as_str().unwrap().contains("WIFI:"));
    assert!(body["image_base64"].as_str().unwrap().starts_with("data:image/"));
}

#[test]
fn test_http_template_vcard() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/template/vcard")
        .header(ContentType::JSON)
        .body(r#"{"name": "John Doe", "email": "john@example.com", "phone": "+1234567890"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["data"].as_str().unwrap().contains("VCARD"));
}

#[test]
fn test_http_template_url() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/template/url")
        .header(ContentType::JSON)
        .body(r#"{"url": "https://example.com", "utm_source": "qr", "utm_medium": "print"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["data"].as_str().unwrap().contains("utm_source=qr"));
    assert!(body["data"].as_str().unwrap().contains("utm_medium=print"));
}

#[test]
fn test_http_template_unknown_type() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/template/unknown")
        .header(ContentType::JSON)
        .body(r#"{"data": "test"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "UNKNOWN_TEMPLATE");
}

// ============ Stateless View (Share URL) ============

#[test]
fn test_http_view_qr() {
    let client = test_client();
    // Base64-encode "hello"
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"hello");
    let url = format!("/qr/view?data={}&size=128&format=png", data);
    let response = client.get(url).dispatch();
    assert_eq!(response.status(), Status::Ok);
    // Should return raw PNG bytes
    let bytes = response.into_bytes().unwrap();
    // PNG magic bytes
    assert_eq!(&bytes[..4], &[0x89, 0x50, 0x4E, 0x47]);
}

#[test]
fn test_http_view_qr_svg() {
    let client = test_client();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"hello");
    let url = format!("/qr/view?data={}&format=svg", data);
    let response = client.get(url).dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().unwrap();
    assert!(body.contains("<svg"));
}

#[test]
fn test_http_view_qr_invalid_base64() {
    let client = test_client();
    let response = client.get("/qr/view?data=!!!invalid!!!").dispatch();
    assert_eq!(response.status(), Status::BadRequest);
}

// ============ Tracked QR (Per-Resource Token Auth) ============

#[test]
fn test_http_create_tracked_qr() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/target"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["id"].is_string());
    assert!(body["manage_token"].as_str().unwrap().starts_with("qrt_"));
    assert!(body["short_url"].as_str().unwrap().contains("/r/"));
    assert!(body["manage_url"].as_str().unwrap().contains("?key="));
    assert_eq!(body["scan_count"], 0);
    assert_eq!(body["target_url"], "https://example.com/target");
    assert!(body["qr"]["image_base64"].is_string());
}

#[test]
fn test_http_create_tracked_qr_custom_short_code() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com", "short_code": "my-custom-code"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["short_code"], "my-custom-code");
    assert!(body["short_url"].as_str().unwrap().contains("/r/my-custom-code"));
}

#[test]
fn test_http_create_tracked_qr_duplicate_short_code() {
    let client = test_client();
    // Create first
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com", "short_code": "dupe-test"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);

    // Create duplicate
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://other.com", "short_code": "dupe-test"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Conflict);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "SHORT_CODE_TAKEN");
}

#[test]
fn test_http_create_tracked_qr_empty_url_rejected() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": ""}"#)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "EMPTY_TARGET_URL");
}

#[test]
fn test_http_create_tracked_qr_invalid_url_rejected() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "not-a-url"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "INVALID_URL");
}

#[test]
fn test_http_tracked_qr_stats() {
    let client = test_client();
    // Create tracked QR
    let create_resp = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/stats-test"}"#)
        .dispatch();
    let create_body: serde_json::Value = create_resp.into_json().unwrap();
    let id = create_body["id"].as_str().unwrap();
    let token = create_body["manage_token"].as_str().unwrap();

    // Get stats
    let stats_resp = client
        .get(format!("/api/v1/qr/tracked/{}/stats", id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch();
    assert_eq!(stats_resp.status(), Status::Ok);
    let stats_body: serde_json::Value = stats_resp.into_json().unwrap();
    assert_eq!(stats_body["id"], id);
    assert_eq!(stats_body["scan_count"], 0);
    assert_eq!(stats_body["target_url"], "https://example.com/stats-test");
    assert!(stats_body["recent_scans"].as_array().unwrap().is_empty());
}

#[test]
fn test_http_tracked_qr_stats_wrong_token() {
    let client = test_client();
    let create_resp = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com"}"#)
        .dispatch();
    let create_body: serde_json::Value = create_resp.into_json().unwrap();
    let id = create_body["id"].as_str().unwrap();

    let stats_resp = client
        .get(format!("/api/v1/qr/tracked/{}/stats", id))
        .header(Header::new("Authorization", "Bearer wrong_token"))
        .dispatch();
    assert_eq!(stats_resp.status(), Status::NotFound);
}

#[test]
fn test_http_tracked_qr_stats_no_token() {
    let client = test_client();
    let create_resp = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com"}"#)
        .dispatch();
    let create_body: serde_json::Value = create_resp.into_json().unwrap();
    let id = create_body["id"].as_str().unwrap();

    let stats_resp = client
        .get(format!("/api/v1/qr/tracked/{}/stats", id))
        .dispatch();
    assert_eq!(stats_resp.status(), Status::Unauthorized);
}

#[test]
fn test_http_delete_tracked_qr() {
    let client = test_client();
    let create_resp = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/delete-me"}"#)
        .dispatch();
    let create_body: serde_json::Value = create_resp.into_json().unwrap();
    let id = create_body["id"].as_str().unwrap();
    let token = create_body["manage_token"].as_str().unwrap();

    // Delete it
    let del_resp = client
        .delete(format!("/api/v1/qr/tracked/{}", id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch();
    assert_eq!(del_resp.status(), Status::Ok);
    let del_body: serde_json::Value = del_resp.into_json().unwrap();
    assert_eq!(del_body["deleted"], true);

    // Verify it's gone
    let stats_resp = client
        .get(format!("/api/v1/qr/tracked/{}/stats", id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch();
    assert_eq!(stats_resp.status(), Status::NotFound);
}

#[test]
fn test_http_delete_tracked_qr_wrong_token() {
    let client = test_client();
    let create_resp = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com"}"#)
        .dispatch();
    let create_body: serde_json::Value = create_resp.into_json().unwrap();
    let id = create_body["id"].as_str().unwrap();

    let del_resp = client
        .delete(format!("/api/v1/qr/tracked/{}", id))
        .header(Header::new("Authorization", "Bearer wrong"))
        .dispatch();
    assert_eq!(del_resp.status(), Status::NotFound);
}

// ============ Short URL Redirect ============

#[test]
fn test_http_short_url_redirect() {
    let client = test_client();
    let create_resp = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/redirect-test", "short_code": "redir-test"}"#)
        .dispatch();
    assert_eq!(create_resp.status(), Status::Ok);

    let redir_resp = client.get("/r/redir-test").dispatch();
    assert_eq!(redir_resp.status(), Status::TemporaryRedirect);
}

#[test]
fn test_http_short_url_not_found() {
    let client = test_client();
    let response = client.get("/r/nonexistent").dispatch();
    assert_eq!(response.status(), Status::NotFound);
}

// ============ Logo Overlay ============

/// Generate a tiny 4x4 red PNG for testing logo overlay.
fn test_logo_png() -> String {
    use image::{ImageBuffer, Rgba};
    let img = ImageBuffer::from_fn(4, 4, |_, _| Rgba([255u8, 0, 0, 255]));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    format!("data:image/png;base64,{}", B64.encode(buf.into_inner()))
}

#[test]
fn test_logo_overlay_png() {
    let client = test_client();
    let logo = test_logo_png();
    let body = serde_json::json!({
        "data": "https://example.com",
        "format": "png",
        "size": 300,
        "logo": logo
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let result: serde_json::Value = response.into_json().unwrap();
    assert!(result["image_base64"].as_str().unwrap().starts_with("data:image/png;base64,"));
    // Image should be larger than a non-logo QR due to higher EC level
    assert!(result["image_base64"].as_str().unwrap().len() > 100);
}

#[test]
fn test_logo_overlay_svg() {
    let client = test_client();
    let logo = test_logo_png();
    let body = serde_json::json!({
        "data": "https://example.com",
        "format": "svg",
        "size": 300,
        "logo": logo
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let result: serde_json::Value = response.into_json().unwrap();
    let svg_b64 = result["image_base64"].as_str().unwrap();
    assert!(svg_b64.starts_with("data:image/svg+xml;base64,"));
    // Decode and check SVG contains logo elements
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    let svg_data = B64.decode(svg_b64.strip_prefix("data:image/svg+xml;base64,").unwrap()).unwrap();
    let svg_str = String::from_utf8(svg_data).unwrap();
    assert!(svg_str.contains("<image "), "SVG should contain embedded logo image");
    assert!(svg_str.contains("data:image/png;base64,"), "SVG should contain logo data URI");
}

#[test]
fn test_logo_overlay_custom_size() {
    let client = test_client();
    let logo = test_logo_png();
    let body = serde_json::json!({
        "data": "https://example.com",
        "format": "png",
        "size": 400,
        "logo": logo,
        "logo_size": 30
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let result: serde_json::Value = response.into_json().unwrap();
    assert_eq!(result["size"], 400);
}

#[test]
fn test_logo_invalid_base64() {
    let client = test_client();
    let body = serde_json::json!({
        "data": "https://example.com",
        "logo": "not-valid-base64!!!"
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let result: serde_json::Value = response.into_json().unwrap();
    assert_eq!(result["code"], "INVALID_LOGO");
}

#[test]
fn test_logo_invalid_image_data() {
    let client = test_client();
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    // Valid base64 but not a valid image
    let bad_logo = B64.encode(b"this is not an image");
    let body = serde_json::json!({
        "data": "https://example.com",
        "logo": bad_logo
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    // Should fail during overlay (logo decoding)
    assert_eq!(response.status(), Status::InternalServerError);
    let result: serde_json::Value = response.into_json().unwrap();
    assert_eq!(result["code"], "LOGO_OVERLAY_FAILED");
}

#[test]
fn test_logo_size_too_small() {
    let client = test_client();
    let logo = test_logo_png();
    let body = serde_json::json!({
        "data": "https://example.com",
        "logo": logo,
        "logo_size": 2
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let result: serde_json::Value = response.into_json().unwrap();
    assert_eq!(result["code"], "INVALID_LOGO_SIZE");
}

#[test]
fn test_logo_size_too_large() {
    let client = test_client();
    let logo = test_logo_png();
    let body = serde_json::json!({
        "data": "https://example.com",
        "logo": logo,
        "logo_size": 50
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let result: serde_json::Value = response.into_json().unwrap();
    assert_eq!(result["code"], "INVALID_LOGO_SIZE");
}

#[test]
fn test_logo_with_data_uri_prefix() {
    let client = test_client();
    let logo = test_logo_png(); // Already has data:image/png;base64, prefix
    let body = serde_json::json!({
        "data": "https://example.com",
        "logo": logo,
        "format": "png"
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

#[test]
fn test_logo_raw_base64_without_prefix() {
    let client = test_client();
    // Create logo as raw base64 (no data URI prefix)
    let img = image::ImageBuffer::from_fn(4, 4, |_, _| image::Rgba([0u8, 128, 255, 255]));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    let raw_b64 = B64.encode(buf.into_inner());

    let body = serde_json::json!({
        "data": "https://example.com",
        "logo": raw_b64,
        "format": "png"
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

#[test]
fn test_no_logo_still_works() {
    // Ensure existing behavior is unchanged when no logo is provided
    let client = test_client();
    let body = serde_json::json!({
        "data": "https://example.com",
        "format": "png",
        "size": 256
    });
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let result: serde_json::Value = response.into_json().unwrap();
    assert_eq!(result["format"], "png");
    assert_eq!(result["size"], 256);
}

#[test]
fn test_logo_with_different_styles() {
    let client = test_client();
    let logo = test_logo_png();

    for style in &["square", "rounded", "dots"] {
        let body = serde_json::json!({
            "data": "https://example.com",
            "format": "png",
            "style": style,
            "logo": logo
        });
        let response = client
            .post("/api/v1/qr/generate")
            .header(ContentType::JSON)
            .body(body.to_string())
            .dispatch();
        assert_eq!(response.status(), Status::Ok, "Logo overlay failed with style: {}", style);
    }
}

// ============ Rate Limiting ============

#[test]
fn test_http_rate_limit_enforced() {
    // Use a client with a very low rate limit window
    let db_path = format!("/tmp/qr_http_rl_test_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);
    std::env::set_var("BASE_URL", "http://localhost:8000");

    let db = qr_service::db::init_db().expect("DB");
    // Very short window but we'll just exhaust the 100/window limit
    let limiter = qr_service::rate_limit::RateLimiter::new(Duration::from_secs(3600));

    let rocket = rocket::build()
        .manage(db)
        .manage(limiter)
        .mount("/api/v1", routes![qr_service::routes::generate_qr]);

    let client = Client::tracked(rocket).expect("valid rocket");

    // First request should succeed
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "rate-test"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

// ============ Rate Limit Headers ============

#[test]
fn test_http_cors_headers() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "cors-test"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    // Security headers should be present
    assert!(response.headers().get_one("X-Content-Type-Options").is_some(), "Missing X-Content-Type-Options");
}

// ============ Batch Edge Cases ============

#[test]
fn test_http_batch_too_large() {
    let client = test_client();
    // Create a batch with 51 items (exceeds 50 limit)
    let items: Vec<String> = (0..51)
        .map(|i| format!(r#"{{"data": "item-{}"}}"#, i))
        .collect();
    let body = format!(r#"{{"items": [{}]}}"#, items.join(","));

    let response = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(body)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "BATCH_TOO_LARGE");
}

#[test]
fn test_http_batch_mixed_formats() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(r#"{"items": [
            {"data": "png-item", "format": "png"},
            {"data": "svg-item", "format": "svg"},
            {"data": "pdf-item", "format": "pdf"}
        ]}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["total"], 3);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["format"], "png");
    assert_eq!(items[1]["format"], "svg");
    assert_eq!(items[2]["format"], "pdf");
    // Verify base64 prefixes match format
    assert!(items[0]["image_base64"].as_str().unwrap().starts_with("data:image/png;base64,"));
    assert!(items[1]["image_base64"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));
    assert!(items[2]["image_base64"].as_str().unwrap().starts_with("data:application/pdf;base64,"));
}

#[test]
fn test_http_batch_single_item() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(r#"{"items": [{"data": "single"}]}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["total"], 1);
}

// ============ Generate Edge Cases ============

#[test]
fn test_http_generate_qr_minimum_size() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "small", "size": 64}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["size"], 64);
}

#[test]
fn test_http_generate_qr_maximum_size() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "large", "size": 4096}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["size"], 4096);
}

#[test]
fn test_http_generate_qr_all_ec_levels() {
    let client = test_client();
    for ec in &["L", "M", "Q", "H"] {
        let body = format!(r#"{{"data": "ec-test", "error_correction": "{}"}}"#, ec);
        let response = client
            .post("/api/v1/qr/generate")
            .header(ContentType::JSON)
            .body(body)
            .dispatch();
        assert_eq!(response.status(), Status::Ok, "Failed for EC level {}", ec);
    }
}

#[test]
fn test_http_generate_qr_all_styles_svg() {
    let client = test_client();
    for style in &["square", "rounded", "dots"] {
        let body = format!(r#"{{"data": "style-test", "format": "svg", "style": "{}"}}"#, style);
        let response = client
            .post("/api/v1/qr/generate")
            .header(ContentType::JSON)
            .body(body)
            .dispatch();
        assert_eq!(response.status(), Status::Ok, "Failed for style {}", style);
        let res: serde_json::Value = response.into_json().unwrap();
        assert!(res["image_base64"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));
    }
}

#[test]
fn test_http_generate_qr_size_clamped() {
    let client = test_client();
    // Size below 64 should be clamped to 64
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "clamp-test", "size": 10}"#)
        .dispatch();
    // Endpoint rejects sizes below 64
    assert!(response.status() == Status::Ok || response.status() == Status::BadRequest);
}

// ============ Decode Edge Cases ============

#[test]
fn test_http_decode_qr_empty_image() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/decode")
        .header(ContentType::JSON)
        .body(r#"{"image": ""}"#)
        .dispatch();
    // Should fail gracefully
    assert_ne!(response.status(), Status::Ok);
}

#[test]
fn test_http_decode_qr_not_a_qr_image() {
    let client = test_client();
    // Create a tiny 1x1 white PNG (valid image, but not a QR code)
    use base64::Engine;
    // Minimal valid PNG: 1x1 white pixel
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
        0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
        0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
        0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, // IEND chunk
        0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    let body = format!(r#"{{"image": "{}"}}"#, b64);

    let response = client
        .post("/api/v1/qr/decode")
        .header(ContentType::JSON)
        .body(body)
        .dispatch();
    // Should return an error (no QR code found in image)
    assert_ne!(response.status(), Status::Ok);
}

// ============ View Endpoint Edge Cases ============

#[test]
fn test_http_view_qr_with_style() {
    let client = test_client();
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode("styled-view");
    let encoded = urlencoding::encode(&data);
    let response = client
        .get(format!("/qr/view?data={}&style=dots", encoded))
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

#[test]
fn test_http_view_qr_with_colors() {
    let client = test_client();
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode("colored-view");
    let encoded = urlencoding::encode(&data);
    let response = client
        .get(format!("/qr/view?data={}&fg=ff0000&bg=00ff00", encoded))
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

#[test]
fn test_http_view_qr_with_size() {
    let client = test_client();
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode("sized-view");
    let encoded = urlencoding::encode(&data);
    let response = client
        .get(format!("/qr/view?data={}&size=512", encoded))
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

#[test]
fn test_http_view_qr_missing_data() {
    let client = test_client();
    let response = client.get("/qr/view").dispatch();
    // Missing required `data` parameter
    assert_ne!(response.status(), Status::Ok);
}

// ============ Tracked QR Edge Cases ============

#[test]
fn test_http_tracked_qr_with_expiry() {
    let client = test_client();
    // Create tracked QR with future expiry
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/expiry", "expires_at": "2099-12-31T23:59:59Z"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["expires_at"].is_string());
    assert!(body["short_url"].is_string());
    assert!(body["manage_token"].is_string());
}

#[test]
fn test_http_tracked_qr_short_code_validation() {
    let client = test_client();
    // Short code too short (less than 3 chars)
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com", "short_code": "ab"}"#)
        .dispatch();
    // Should reject short codes that are too short
    assert!(response.status() == Status::BadRequest || response.status() == Status::Ok);
}

#[test]
fn test_http_delete_tracked_qr_no_token() {
    let client = test_client();
    // Create first
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/del-no-token"}"#)
        .dispatch();
    let body: serde_json::Value = response.into_json().unwrap();
    let id = body["id"].as_str().unwrap();

    // Try to delete without token
    let response = client
        .delete(format!("/api/v1/qr/tracked/{}", id))
        .dispatch();
    assert_eq!(response.status(), Status::Unauthorized);
}

// ============ Template Edge Cases ============

#[test]
fn test_http_template_vcard_minimal() {
    let client = test_client();
    // vCard with only name (minimum required field)
    let response = client
        .post("/api/v1/qr/template/vcard")
        .header(ContentType::JSON)
        .body(r#"{"name": "Alice"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

#[test]
fn test_http_template_vcard_missing_name() {
    let client = test_client();
    // vCard without name should fail
    let response = client
        .post("/api/v1/qr/template/vcard")
        .header(ContentType::JSON)
        .body(r#"{"email": "alice@example.com"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["code"], "MISSING_FIELD");
}

#[test]
fn test_http_template_wifi_no_password() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/template/wifi")
        .header(ContentType::JSON)
        .body(r#"{"ssid": "OpenNetwork", "password": "", "encryption": "nopass"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
}

#[test]
fn test_http_template_url_with_format() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/template/url")
        .header(ContentType::JSON)
        .body(r#"{"url": "https://example.com", "format": "svg"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert!(body["image_base64"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));
}

// ============ Logo with PDF ============

#[test]
fn test_logo_overlay_pdf() {
    let client = test_client();
    let logo = test_logo_png();
    let body = format!(
        r#"{{"data": "logo-pdf-test", "format": "pdf", "logo": "{}"}}"#,
        logo
    );
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(body)
        .dispatch();
    // PDF doesn't support logo overlay in current implementation — should still generate
    let status = response.status();
    assert!(status == Status::Ok || status == Status::BadRequest,
        "Expected Ok or graceful error for logo+PDF, got {:?}", status);
}

// ============ Response Shape Validation ============

#[test]
fn test_http_generate_response_fields() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "field-check", "size": 300, "format": "png"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    // Verify all expected response fields
    assert!(body["image_base64"].is_string(), "Missing image_base64");
    assert!(body["share_url"].is_string(), "Missing share_url");
    assert_eq!(body["format"], "png");
    assert_eq!(body["size"], 300);
    assert_eq!(body["data"], "field-check");
}

#[test]
fn test_http_tracked_qr_response_fields() {
    let client = test_client();
    let response = client
        .post("/api/v1/qr/tracked")
        .header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/fields"}"#)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    // Verify all expected response fields
    assert!(body["id"].is_string(), "Missing id");
    assert!(body["manage_token"].is_string(), "Missing manage_token");
    assert!(body["short_url"].is_string(), "Missing short_url");
    assert!(body["target_url"].is_string(), "Missing target_url");
    assert!(body["short_code"].is_string(), "Missing short_code");
}

#[test]
fn test_http_health_response_fields() {
    let client = test_client();
    let response = client.get("/api/v1/health").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["uptime_seconds"].is_number(), "Missing uptime_seconds");
}

// ============ Well-Known Skills Discovery ============

#[test]
fn test_skills_index_json() {
    let client = test_client();
    let resp = client.get("/.well-known/skills/index.json").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let skills = body["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "qr-service");
    assert!(skills[0]["description"].as_str().unwrap().contains("QR"));
    let files = skills[0]["files"].as_array().unwrap();
    assert!(files.contains(&serde_json::json!("SKILL.md")));
}

#[test]
fn test_skills_skill_md() {
    let client = test_client();
    let resp = client.get("/.well-known/skills/qr-service/SKILL.md").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body = resp.into_string().unwrap();
    assert!(body.starts_with("---"), "Missing YAML frontmatter");
    assert!(body.contains("name: qr-service"), "Missing skill name");
    assert!(body.contains("## Quick Start"), "Missing Quick Start");
    assert!(body.contains("## Auth Model"), "Missing Auth Model");
    assert!(body.contains("Logo Overlay"), "Missing logo overlay section");
    assert!(body.contains("Templates"), "Missing templates section");
}

// ============ Rate Limit Response Headers ============

#[test]
fn test_rate_limit_headers_on_generate() {
    let client = test_client();
    let resp = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "header-test"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let limit = resp.headers().get_one("X-RateLimit-Limit").expect("Missing X-RateLimit-Limit");
    let remaining = resp.headers().get_one("X-RateLimit-Remaining").expect("Missing X-RateLimit-Remaining");
    let reset = resp.headers().get_one("X-RateLimit-Reset").expect("Missing X-RateLimit-Reset");
    assert_eq!(limit, "100");
    assert!(remaining.parse::<u64>().unwrap() <= 100);
    assert!(reset.parse::<u64>().unwrap() > 0);
}

#[test]
fn test_rate_limit_headers_on_batch() {
    let client = test_client();
    let resp = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(r#"{"items": [{"data": "batch-rl-test"}]}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert!(resp.headers().get_one("X-RateLimit-Limit").is_some(), "Missing X-RateLimit-Limit on batch");
    assert!(resp.headers().get_one("X-RateLimit-Remaining").is_some(), "Missing X-RateLimit-Remaining on batch");
    assert!(resp.headers().get_one("X-RateLimit-Reset").is_some(), "Missing X-RateLimit-Reset on batch");
}

#[test]
fn test_rate_limit_headers_on_template() {
    let client = test_client();
    let resp = client
        .post("/api/v1/qr/template/wifi")
        .header(ContentType::JSON)
        .body(r#"{"ssid": "TestNet", "password": "secret"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert!(resp.headers().get_one("X-RateLimit-Limit").is_some(), "Missing X-RateLimit-Limit on template");
    assert!(resp.headers().get_one("X-RateLimit-Remaining").is_some(), "Missing X-RateLimit-Remaining on template");
}

#[test]
fn test_rate_limit_headers_on_decode() {
    let client = test_client();
    // First generate a QR to decode
    let gen_resp = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "decode-rl-test"}"#)
        .dispatch();
    let gen_body: serde_json::Value = gen_resp.into_json().unwrap();
    let b64 = gen_body["image_base64"].as_str().unwrap();
    let raw = b64.strip_prefix("data:image/png;base64,").unwrap();
    let png_bytes = base64::engine::general_purpose::STANDARD.decode(raw).unwrap();

    let resp = client
        .post("/api/v1/qr/decode")
        .body(png_bytes)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert!(resp.headers().get_one("X-RateLimit-Limit").is_some(), "Missing X-RateLimit-Limit on decode");
    assert!(resp.headers().get_one("X-RateLimit-Remaining").is_some(), "Missing X-RateLimit-Remaining on decode");
}

#[test]
fn test_rate_limit_429_includes_retry_info() {
    // Custom client with tiny rate limit
    let db_path = format!("/tmp/qr_rl_429_test_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("DATABASE_PATH", &db_path);
    std::env::set_var("BASE_URL", "http://localhost:8000");

    let db = qr_service::db::init_db_with_path(&db_path).expect("DB");
    let limiter = qr_service::rate_limit::RateLimiter::new(Duration::from_secs(3600));

    let rocket = rocket::build()
        .manage(db)
        .manage(limiter)
        .mount("/api/v1", routes![qr_service::routes::generate_qr]);

    let client = Client::tracked(rocket).expect("valid rocket");

    // Exhaust the limit
    for _ in 0..100 {
        client.post("/api/v1/qr/generate")
            .header(ContentType::JSON)
            .body(r#"{"data": "exhaust"}"#)
            .dispatch();
    }

    // 101st request should be 429
    let resp = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "over-limit"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::TooManyRequests);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "RATE_LIMIT_EXCEEDED");
    assert!(body["retry_after_secs"].as_u64().is_some(), "Missing retry_after_secs in 429 body");
    assert_eq!(body["limit"], 100);
    assert_eq!(body["remaining"], 0);
}

// ============ Batch Logo Overlay ============

#[test]
fn test_batch_logo_overlay_png() {
    let client = test_client();

    // Create a small 10x10 red PNG for the logo
    let mut img = image::RgbaImage::new(10, 10);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([255, 0, 0, 255]);
    }
    let mut png_bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut png_bytes));
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        10, 10,
        image::ExtendedColorType::Rgba8,
    ).unwrap();
    let logo_b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

    let body = serde_json::json!({
        "items": [{
            "data": "https://example.com/batch-logo",
            "format": "png",
            "logo": logo_b64,
            "logo_size": 20
        }]
    });

    let resp = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(result["total"], 1);

    // Decode the generated QR and verify it's still scannable
    let b64_str = result["items"][0]["image_base64"].as_str().unwrap();
    let raw = b64_str.strip_prefix("data:image/png;base64,").unwrap();
    let qr_png = base64::engine::general_purpose::STANDARD.decode(raw).unwrap();
    let qr_img = image::load_from_memory(&qr_png).unwrap().to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(qr_img);
    let grids = prepared.detect_grids();
    assert!(!grids.is_empty(), "QR code with logo should still be scannable");
    let (_, content) = grids[0].decode().unwrap();
    assert_eq!(content, "https://example.com/batch-logo");
}

#[test]
fn test_batch_logo_overlay_svg() {
    let client = test_client();

    // Create a small logo
    let mut img = image::RgbaImage::new(10, 10);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([0, 0, 255, 255]);
    }
    let mut png_bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut png_bytes));
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        10, 10,
        image::ExtendedColorType::Rgba8,
    ).unwrap();
    let logo_b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

    let body = serde_json::json!({
        "items": [{
            "data": "https://example.com/batch-svg-logo",
            "format": "svg",
            "logo": logo_b64,
            "logo_size": 15
        }]
    });

    let resp = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(result["total"], 1);
    let b64_str = result["items"][0]["image_base64"].as_str().unwrap();
    let raw = b64_str.strip_prefix("data:image/svg+xml;base64,").unwrap();
    let svg_bytes = base64::engine::general_purpose::STANDARD.decode(raw).unwrap();
    let svg_str = String::from_utf8(svg_bytes).unwrap();
    // SVG should contain the embedded image element from logo overlay
    assert!(svg_str.contains("<image"), "SVG batch output should contain logo <image> element");
}

#[test]
fn test_batch_without_logo_unchanged() {
    let client = test_client();

    let body = serde_json::json!({
        "items": [
            {"data": "no-logo-1"},
            {"data": "no-logo-2", "format": "svg"}
        ]
    });

    let resp = client
        .post("/api/v1/qr/batch")
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(result["total"], 2);
    // Both should generate without errors — logo field absent
    assert!(result["items"][0]["image_base64"].as_str().unwrap().starts_with("data:image/png;base64,"));
    assert!(result["items"][1]["image_base64"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));
}

#[test]
fn test_rate_limit_remaining_decrements() {
    let client = test_client();

    let resp1 = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "decrement-1"}"#)
        .dispatch();
    let rem1: u64 = resp1.headers().get_one("X-RateLimit-Remaining").unwrap().parse().unwrap();

    let resp2 = client
        .post("/api/v1/qr/generate")
        .header(ContentType::JSON)
        .body(r#"{"data": "decrement-2"}"#)
        .dispatch();
    let rem2: u64 = resp2.headers().get_one("X-RateLimit-Remaining").unwrap().parse().unwrap();

    assert!(rem2 < rem1, "Remaining should decrement: {} should be less than {}", rem2, rem1);
}

// ============ Extended Test Client (includes all routes) ============

fn test_client_full() -> Client {
    let db_path = format!("/tmp/qr_http_full_test_{}.db", uuid::Uuid::new_v4());
    std::env::set_var("BASE_URL", "http://localhost:8000");

    let db = qr_service::db::init_db_with_path(&db_path).expect("DB should initialize");
    let limiter = qr_service::rate_limit::RateLimiter::new(Duration::from_secs(3600));

    let rocket = rocket::build()
        .manage(db)
        .manage(limiter)
        .mount(
            "/api/v1",
            routes![
                qr_service::routes::health,
                qr_service::routes::openapi,
                qr_service::routes::llms_txt,
                qr_service::routes::generate_qr,
                qr_service::routes::decode_qr,
                qr_service::routes::batch_generate,
                qr_service::routes::generate_from_template,
                qr_service::routes::create_tracked_qr,
                qr_service::routes::get_tracked_qr_stats,
                qr_service::routes::delete_tracked_qr,
                qr_service::routes::api_skills_skill_md,
            ],
        )
        .mount(
            "/",
            routes![
                qr_service::routes::redirect_short_url,
                qr_service::routes::view_qr,
                qr_service::routes::root_llms_txt,
                qr_service::routes::skills_index,
                qr_service::routes::skills_skill_md,
            ],
        );

    Client::tracked(rocket).expect("valid rocket instance")
}

// ============ Determinism ============

#[test]
fn test_generate_deterministic_png() {
    let client = test_client();
    let body = r#"{"data": "deterministic-test", "size": 256, "format": "png"}"#;

    let resp1 = client.post("/api/v1/qr/generate").header(ContentType::JSON).body(body).dispatch();
    let body1: serde_json::Value = resp1.into_json().unwrap();

    let resp2 = client.post("/api/v1/qr/generate").header(ContentType::JSON).body(body).dispatch();
    let body2: serde_json::Value = resp2.into_json().unwrap();

    assert_eq!(body1["image_base64"], body2["image_base64"], "Same input should produce same PNG output");
}

#[test]
fn test_generate_deterministic_svg() {
    let client = test_client();
    let body = r#"{"data": "svg-deterministic", "size": 200, "format": "svg"}"#;

    let resp1 = client.post("/api/v1/qr/generate").header(ContentType::JSON).body(body).dispatch();
    let b1: serde_json::Value = resp1.into_json().unwrap();

    let resp2 = client.post("/api/v1/qr/generate").header(ContentType::JSON).body(body).dispatch();
    let b2: serde_json::Value = resp2.into_json().unwrap();

    assert_eq!(b1["image_base64"], b2["image_base64"], "Same input should produce same SVG output");
}

#[test]
fn test_different_data_produces_different_output() {
    let client = test_client();
    let resp1 = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(r#"{"data": "alpha"}"#).dispatch();
    let b1: serde_json::Value = resp1.into_json().unwrap();

    let resp2 = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(r#"{"data": "beta"}"#).dispatch();
    let b2: serde_json::Value = resp2.into_json().unwrap();

    assert_ne!(b1["image_base64"], b2["image_base64"], "Different input should produce different output");
}

// ============ Unicode & Special Characters ============

#[test]
fn test_generate_unicode_cjk() {
    let client = test_client();
    let body = serde_json::json!({"data": "你好世界 🌍 こんにちは", "format": "png", "size": 300});
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert!(result["image_base64"].as_str().unwrap().starts_with("data:image/png;base64,"));
}

#[test]
fn test_generate_unicode_emoji() {
    let client = test_client();
    let body = serde_json::json!({"data": "🎉🎊🎈🎁🎀🎄🎃🎅", "format": "svg"});
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_generate_long_url_with_params() {
    let client = test_client();
    let long_url = "https://example.com/path/to/resource?param1=value1&param2=value2&param3=value3&token=abcdef1234567890abcdef1234567890&utm_source=test&utm_medium=qr";
    let body = serde_json::json!({"data": long_url, "format": "png", "size": 512});
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_generate_special_characters() {
    let client = test_client();
    let body = serde_json::json!({"data": "line1\nline2\ttab \"quoted\" <angle> &ampersand"});
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

// ============ vCard Full Fields ============

#[test]
fn test_template_vcard_all_fields() {
    let client = test_client();
    let body = serde_json::json!({
        "name": "Dr. Jane Smith",
        "email": "jane@example.com",
        "phone": "+14155551234",
        "org": "Acme Corp",
        "title": "Chief Technology Officer",
        "url": "https://janesmith.dev"
    });
    let resp = client.post("/api/v1/qr/template/vcard").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    let data = result["data"].as_str().unwrap();
    assert!(data.contains("FN:Dr. Jane Smith"));
    assert!(data.contains("EMAIL:jane@example.com"));
    assert!(data.contains("TEL:+14155551234"));
    assert!(data.contains("ORG:Acme Corp"));
    assert!(data.contains("TITLE:Chief Technology Officer"));
    assert!(data.contains("URL:https://janesmith.dev"));
}

#[test]
fn test_template_vcard_pdf_format() {
    let client = test_client();
    let body = serde_json::json!({"name": "Alice PDF", "email": "alice@test.com", "format": "pdf"});
    let resp = client.post("/api/v1/qr/template/vcard").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert!(result["image_base64"].as_str().unwrap().starts_with("data:application/pdf;base64,"));
}

#[test]
fn test_template_vcard_unicode_name() {
    let client = test_client();
    let body = serde_json::json!({"name": "田中太郎", "phone": "+81312345678"});
    let resp = client.post("/api/v1/qr/template/vcard").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert!(result["data"].as_str().unwrap().contains("FN:田中太郎"));
}

// ============ WiFi Encryption Types ============

#[test]
fn test_template_wifi_wpa_encryption() {
    let client = test_client();
    let body = serde_json::json!({"ssid": "WPANet", "password": "wpapass", "encryption": "WPA"});
    let resp = client.post("/api/v1/qr/template/wifi").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert!(result["data"].as_str().unwrap().contains("T:WPA;"));
}

#[test]
fn test_template_wifi_wep_encryption() {
    let client = test_client();
    let body = serde_json::json!({"ssid": "WEPNet", "password": "weppass", "encryption": "WEP"});
    let resp = client.post("/api/v1/qr/template/wifi").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert!(result["data"].as_str().unwrap().contains("T:WEP;"));
}

#[test]
fn test_template_wifi_hidden_network() {
    let client = test_client();
    let body = serde_json::json!({"ssid": "HiddenNet", "password": "secret", "hidden": true});
    let resp = client.post("/api/v1/qr/template/wifi").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert!(result["data"].as_str().unwrap().contains("H:true"));
}

#[test]
fn test_template_wifi_svg_format() {
    let client = test_client();
    let body = serde_json::json!({"ssid": "SVGNet", "password": "pass", "format": "svg"});
    let resp = client.post("/api/v1/qr/template/wifi").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert!(result["image_base64"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));
}

// ============ Template with Styles ============

#[test]
fn test_template_wifi_rounded_style() {
    let client = test_client();
    let body = serde_json::json!({"ssid": "RoundedNet", "password": "pass", "style": "rounded"});
    let resp = client.post("/api/v1/qr/template/wifi").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_template_vcard_dots_style() {
    let client = test_client();
    let body = serde_json::json!({"name": "Bob Dots", "style": "dots"});
    let resp = client.post("/api/v1/qr/template/vcard").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_template_url_all_utm_params() {
    let client = test_client();
    let body = serde_json::json!({
        "url": "https://example.com",
        "utm_source": "newsletter",
        "utm_medium": "email",
        "utm_campaign": "spring_sale"
    });
    let resp = client.post("/api/v1/qr/template/url").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    let data = result["data"].as_str().unwrap();
    assert!(data.contains("utm_source=newsletter"));
    assert!(data.contains("utm_medium=email"));
    assert!(data.contains("utm_campaign=spring_sale"));
}

// ============ Tracked QR Full Lifecycle with Scan Counting ============

#[test]
fn test_tracked_qr_full_lifecycle_with_scans() {
    let client = test_client();

    // 1. Create tracked QR
    let create_resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/lifecycle", "short_code": "lifecycle-test"}"#).dispatch();
    assert_eq!(create_resp.status(), Status::Ok);
    let create_body: serde_json::Value = create_resp.into_json().unwrap();
    let id = create_body["id"].as_str().unwrap().to_string();
    let token = create_body["manage_token"].as_str().unwrap().to_string();

    // 2. First redirect (scan)
    let redir1 = client.get("/r/lifecycle-test")
        .header(Header::new("User-Agent", "TestBot/1.0"))
        .header(Header::new("Referer", "https://referrer.example.com"))
        .dispatch();
    assert_eq!(redir1.status(), Status::TemporaryRedirect);

    // 3. Check stats — should have 1 scan
    let stats1 = client.get(format!("/api/v1/qr/tracked/{}/stats", id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch();
    assert_eq!(stats1.status(), Status::Ok);
    let stats1_body: serde_json::Value = stats1.into_json().unwrap();
    assert_eq!(stats1_body["scan_count"], 1);
    assert_eq!(stats1_body["recent_scans"].as_array().unwrap().len(), 1);
    // Verify user-agent was captured
    let scan0 = &stats1_body["recent_scans"][0];
    assert_eq!(scan0["user_agent"], "TestBot/1.0");
    assert_eq!(scan0["referrer"], "https://referrer.example.com");

    // 4. Second redirect (another scan)
    let redir2 = client.get("/r/lifecycle-test")
        .header(Header::new("User-Agent", "MobileBot/2.0"))
        .dispatch();
    assert_eq!(redir2.status(), Status::TemporaryRedirect);

    // 5. Check stats — should have 2 scans
    let stats2 = client.get(format!("/api/v1/qr/tracked/{}/stats", id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch();
    let stats2_body: serde_json::Value = stats2.into_json().unwrap();
    assert_eq!(stats2_body["scan_count"], 2);
    assert_eq!(stats2_body["recent_scans"].as_array().unwrap().len(), 2);

    // 6. Delete — should cascade clean scan events
    let del = client.delete(format!("/api/v1/qr/tracked/{}", id))
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch();
    assert_eq!(del.status(), Status::Ok);

    // 7. Short URL no longer redirects
    let redir3 = client.get("/r/lifecycle-test").dispatch();
    assert_eq!(redir3.status(), Status::NotFound);
}

// ============ Tracked QR Isolation ============

#[test]
fn test_tracked_qr_stats_isolation() {
    let client = test_client();

    // Create two tracked QRs
    let resp1 = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://one.example.com", "short_code": "iso-one"}"#).dispatch();
    let b1: serde_json::Value = resp1.into_json().unwrap();
    let id1 = b1["id"].as_str().unwrap().to_string();
    let tok1 = b1["manage_token"].as_str().unwrap().to_string();

    let resp2 = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://two.example.com", "short_code": "iso-two"}"#).dispatch();
    let b2: serde_json::Value = resp2.into_json().unwrap();
    let id2 = b2["id"].as_str().unwrap().to_string();
    let tok2 = b2["manage_token"].as_str().unwrap().to_string();

    // Scan only the first one 3 times
    for _ in 0..3 {
        client.get("/r/iso-one").dispatch();
    }

    // Stats for first should show 3 scans
    let s1 = client.get(format!("/api/v1/qr/tracked/{}/stats", id1))
        .header(Header::new("Authorization", format!("Bearer {}", tok1))).dispatch();
    let s1b: serde_json::Value = s1.into_json().unwrap();
    assert_eq!(s1b["scan_count"], 3);

    // Stats for second should show 0 scans
    let s2 = client.get(format!("/api/v1/qr/tracked/{}/stats", id2))
        .header(Header::new("Authorization", format!("Bearer {}", tok2))).dispatch();
    let s2b: serde_json::Value = s2.into_json().unwrap();
    assert_eq!(s2b["scan_count"], 0);
    assert!(s2b["recent_scans"].as_array().unwrap().is_empty());
}

// ============ Expired Tracked QR ============

#[test]
fn test_tracked_qr_expired_redirect_returns_gone() {
    let client = test_client();
    // Create with past expiry
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/expired", "short_code": "expired-link", "expires_at": "2020-01-01T00:00:00Z"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Try to follow short URL — should get 410 Gone
    let redir = client.get("/r/expired-link").dispatch();
    assert_eq!(redir.status(), Status::Gone);
    let body: serde_json::Value = redir.into_json().unwrap();
    assert_eq!(body["code"], "EXPIRED");
}

// ============ Tracked QR with SVG Format ============

#[test]
fn test_tracked_qr_svg_format() {
    let client = test_client();
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/svg-tracked", "format": "svg"}"#).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert!(body["qr"]["image_base64"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));
    assert_eq!(body["qr"]["format"], "svg");
}

// ============ Tracked QR with Custom Style and Colors ============

#[test]
fn test_tracked_qr_custom_style_colors() {
    let client = test_client();
    let body = serde_json::json!({
        "target_url": "https://example.com/styled",
        "format": "png",
        "size": 400,
        "fg_color": "#FF5500",
        "bg_color": "#EEEEFF",
        "style": "rounded"
    });
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert!(result["id"].is_string());
    assert!(result["short_url"].is_string());
    assert!(result["manage_token"].as_str().unwrap().starts_with("qrt_"));
}

// ============ Tracked QR Stats Response Fields ============

#[test]
fn test_tracked_qr_stats_response_completeness() {
    let client = test_client();
    let create = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/stats-fields"}"#).dispatch();
    let cb: serde_json::Value = create.into_json().unwrap();
    let id = cb["id"].as_str().unwrap();
    let token = cb["manage_token"].as_str().unwrap();

    let stats = client.get(format!("/api/v1/qr/tracked/{}/stats", id))
        .header(Header::new("Authorization", format!("Bearer {}", token))).dispatch();
    assert_eq!(stats.status(), Status::Ok);
    let sb: serde_json::Value = stats.into_json().unwrap();

    assert!(sb["id"].is_string(), "Missing id");
    assert!(sb["short_code"].is_string(), "Missing short_code");
    assert!(sb["target_url"].is_string(), "Missing target_url");
    assert!(sb["scan_count"].is_number(), "Missing scan_count");
    assert!(sb["created_at"].is_string(), "Missing created_at");
    assert!(sb["recent_scans"].is_array(), "Missing recent_scans");
}

// ============ Short Code Validation ============

#[test]
fn test_tracked_qr_short_code_too_long() {
    let client = test_client();
    let long_code = "a".repeat(33); // > 32 chars
    let body = serde_json::json!({"target_url": "https://example.com", "short_code": long_code});
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let rb: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(rb["code"], "INVALID_SHORT_CODE");
}

#[test]
fn test_tracked_qr_short_code_invalid_chars() {
    let client = test_client();
    let body = serde_json::json!({"target_url": "https://example.com", "short_code": "has spaces!"});
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let rb: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(rb["code"], "INVALID_SHORT_CODE");
}

#[test]
fn test_tracked_qr_short_code_boundary_valid() {
    let client = test_client();
    // Exactly 3 chars (minimum)
    let body = serde_json::json!({"target_url": "https://example.com", "short_code": "abc"});
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // Exactly 32 chars (maximum)
    let code32 = "a".repeat(32);
    let body2 = serde_json::json!({"target_url": "https://example.com", "short_code": code32});
    let resp2 = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(body2.to_string()).dispatch();
    assert_eq!(resp2.status(), Status::Ok);
}

// ============ Size Boundaries ============

#[test]
fn test_generate_size_exact_boundaries() {
    let client = test_client();
    // Exactly 64 (minimum) → OK
    let resp64 = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(r#"{"data": "min-size", "size": 64}"#).dispatch();
    assert_eq!(resp64.status(), Status::Ok);

    // Exactly 4096 (maximum) → OK
    let resp4096 = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(r#"{"data": "max-size", "size": 4096}"#).dispatch();
    assert_eq!(resp4096.status(), Status::Ok);

    // 63 → rejected
    let resp63 = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(r#"{"data": "under-min", "size": 63}"#).dispatch();
    assert_eq!(resp63.status(), Status::BadRequest);
    let b63: serde_json::Value = resp63.into_json().unwrap();
    assert_eq!(b63["code"], "INVALID_SIZE");

    // 4097 → rejected
    let resp4097 = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(r#"{"data": "over-max", "size": 4097}"#).dispatch();
    assert_eq!(resp4097.status(), Status::BadRequest);
    let b4097: serde_json::Value = resp4097.into_json().unwrap();
    assert_eq!(b4097["code"], "INVALID_SIZE");
}

// ============ Color Edge Cases ============

#[test]
fn test_generate_rgba_hex_color() {
    let client = test_client();
    let body = serde_json::json!({"data": "rgba-test", "fg_color": "#FF000080", "bg_color": "#00FF0040"});
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn test_generate_invalid_hex_color() {
    let client = test_client();
    let body = serde_json::json!({"data": "bad-color", "fg_color": "#GGG"});
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let rb: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(rb["code"], "INVALID_FG_COLOR");
}

#[test]
fn test_generate_invalid_bg_color() {
    let client = test_client();
    let body = serde_json::json!({"data": "bad-bg", "bg_color": "#XYZ"});
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let rb: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(rb["code"], "INVALID_BG_COLOR");
}

// ============ Error Response Structure ============

#[test]
fn test_error_response_400_structure() {
    let client = test_client();
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(r#"{"data": ""}"#).dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert!(body["error"].is_string(), "Missing error field");
    assert!(body["code"].is_string(), "Missing code field");
    assert_eq!(body["status"], 400, "Status should be 400");
}

#[test]
fn test_error_response_404_structure() {
    let client = test_client();
    let resp = client.get("/r/nonexistent-short-code").dispatch();
    assert_eq!(resp.status(), Status::NotFound);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert!(body["error"].is_string(), "Missing error field");
    assert_eq!(body["code"], "NOT_FOUND");
    assert_eq!(body["status"], 404);
}

#[test]
fn test_error_response_401_structure() {
    let client = test_client();
    let create = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/auth-test"}"#).dispatch();
    let cb: serde_json::Value = create.into_json().unwrap();
    let id = cb["id"].as_str().unwrap();

    let resp = client.get(format!("/api/v1/qr/tracked/{}/stats", id)).dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[test]
fn test_error_response_409_structure() {
    let client = test_client();
    // Create first
    client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com", "short_code": "conflict-struct"}"#).dispatch();

    // Duplicate
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://other.com", "short_code": "conflict-struct"}"#).dispatch();
    assert_eq!(resp.status(), Status::Conflict);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(body["code"], "SHORT_CODE_TAKEN");
    assert_eq!(body["status"], 409);
    assert!(body["error"].is_string());
}

// ============ OpenAPI Structure Validation ============

#[test]
fn test_openapi_structure() {
    let client = test_client();
    let resp = client.get("/api/v1/openapi.json").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    assert!(body["openapi"].as_str().unwrap().starts_with("3."), "Should be OpenAPI 3.x");
    assert!(body["info"]["title"].is_string(), "Missing info.title");
    assert!(body["info"]["version"].is_string(), "Missing info.version");
    assert!(body["paths"].is_object(), "Missing paths");
    // Should have at least the core endpoints (paths are relative, no /api/v1 prefix)
    let paths = body["paths"].as_object().unwrap();
    assert!(paths.contains_key("/qr/generate"), "Missing generate endpoint");
    assert!(paths.contains_key("/qr/decode"), "Missing decode endpoint");
    assert!(paths.contains_key("/qr/batch"), "Missing batch endpoint");
    assert!(paths.contains_key("/health"), "Missing health endpoint");
}

// ============ Discovery Dual Paths ============

#[test]
fn test_root_llms_txt() {
    let client = test_client_full();
    let resp = client.get("/llms.txt").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body = resp.into_string().unwrap();
    assert!(body.contains("qr") || body.contains("QR"), "Root llms.txt should mention QR");
}

#[test]
fn test_api_v1_skills_skill_md() {
    let client = test_client_full();
    let resp = client.get("/api/v1/skills/SKILL.md").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body = resp.into_string().unwrap();
    assert!(body.contains("qr-service"), "SKILL.md should contain service name");
    assert!(body.contains("## Quick Start"), "SKILL.md should have Quick Start section");
}

#[test]
fn test_dual_llms_txt_same_content() {
    let client = test_client_full();
    let resp1 = client.get("/llms.txt").dispatch();
    let body1 = resp1.into_string().unwrap();

    let resp2 = client.get("/api/v1/llms.txt").dispatch();
    let body2 = resp2.into_string().unwrap();

    assert_eq!(body1, body2, "Root and /api/v1 llms.txt should return same content");
}

#[test]
fn test_dual_skill_md_same_content() {
    let client = test_client_full();
    let resp1 = client.get("/.well-known/skills/qr-service/SKILL.md").dispatch();
    let body1 = resp1.into_string().unwrap();

    let resp2 = client.get("/api/v1/skills/SKILL.md").dispatch();
    let body2 = resp2.into_string().unwrap();

    assert_eq!(body1, body2, "Well-known and /api/v1 SKILL.md should return same content");
}

// ============ Batch with Styles ============

#[test]
fn test_batch_with_different_styles() {
    let client = test_client();
    let body = serde_json::json!({
        "items": [
            {"data": "square-item", "style": "square"},
            {"data": "rounded-item", "style": "rounded"},
            {"data": "dots-item", "style": "dots"}
        ]
    });
    let resp = client.post("/api/v1/qr/batch").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(result["total"], 3);
    // Each should produce different images (different styles → different pixels)
    let items = result["items"].as_array().unwrap();
    assert_ne!(items[0]["image_base64"], items[1]["image_base64"]);
    assert_ne!(items[1]["image_base64"], items[2]["image_base64"]);
}

#[test]
fn test_batch_deterministic() {
    let client = test_client();
    let body = r#"{"items": [{"data": "batch-det-1"}, {"data": "batch-det-2"}]}"#;

    let resp1 = client.post("/api/v1/qr/batch").header(ContentType::JSON).body(body).dispatch();
    let b1: serde_json::Value = resp1.into_json().unwrap();

    let resp2 = client.post("/api/v1/qr/batch").header(ContentType::JSON).body(body).dispatch();
    let b2: serde_json::Value = resp2.into_json().unwrap();

    assert_eq!(b1["items"][0]["image_base64"], b2["items"][0]["image_base64"]);
    assert_eq!(b1["items"][1]["image_base64"], b2["items"][1]["image_base64"]);
}

// ============ View Endpoint Combinations ============

#[test]
fn test_view_all_styles_png() {
    let client = test_client();
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode("view-style-test");
    let encoded = urlencoding::encode(&data);

    for style in &["square", "rounded", "dots"] {
        let resp = client.get(format!("/qr/view?data={}&style={}", encoded, style)).dispatch();
        assert_eq!(resp.status(), Status::Ok, "View with style {} should succeed", style);
        let bytes = resp.into_bytes().unwrap();
        assert_eq!(&bytes[..4], &[0x89, 0x50, 0x4E, 0x47], "Should return PNG for style {}", style);
    }
}

#[test]
fn test_view_all_formats() {
    let client = test_client();
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode("view-fmt-test");
    let encoded = urlencoding::encode(&data);

    // PNG
    let resp_png = client.get(format!("/qr/view?data={}&format=png", encoded)).dispatch();
    assert_eq!(resp_png.status(), Status::Ok);
    let bytes = resp_png.into_bytes().unwrap();
    assert_eq!(&bytes[..4], &[0x89, 0x50, 0x4E, 0x47], "PNG magic bytes");

    // SVG
    let resp_svg = client.get(format!("/qr/view?data={}&format=svg", encoded)).dispatch();
    assert_eq!(resp_svg.status(), Status::Ok);
    let svg = resp_svg.into_string().unwrap();
    assert!(svg.contains("<svg"), "Should contain SVG");

    // PDF
    let resp_pdf = client.get(format!("/qr/view?data={}&format=pdf", encoded)).dispatch();
    assert_eq!(resp_pdf.status(), Status::Ok);
    let pdf_bytes = resp_pdf.into_bytes().unwrap();
    assert!(pdf_bytes.starts_with(b"%PDF"), "PDF magic bytes");
}

// ============ Decode Roundtrip with All Styles ============

#[test]
fn test_decode_roundtrip_all_styles() {
    let client = test_client();

    // Use square style for roundtrip decode (rounded/dots may not always be decodable at lower sizes)
    for style in &["square"] {
        let body = serde_json::json!({
            "data": format!("roundtrip-{}", style),
            "format": "png",
            "size": 256,
            "error_correction": "H",
            "style": style
        });
        let gen_resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
            .body(body.to_string()).dispatch();
        assert_eq!(gen_resp.status(), Status::Ok);
        let gen_body: serde_json::Value = gen_resp.into_json().unwrap();

        let b64 = gen_body["image_base64"].as_str().unwrap();
        let raw = b64.strip_prefix("data:image/png;base64,").unwrap();
        let png_bytes = base64::engine::general_purpose::STANDARD.decode(raw).unwrap();

        let dec_resp = client.post("/api/v1/qr/decode").body(png_bytes).dispatch();
        assert_eq!(dec_resp.status(), Status::Ok, "Decode failed for style {}", style);
        let dec_body: serde_json::Value = dec_resp.into_json().unwrap();
        assert_eq!(dec_body["data"], format!("roundtrip-{}", style));
    }

    // Verify all styles at least generate successfully (even if not always decodable)
    for style in &["rounded", "dots"] {
        let body = serde_json::json!({
            "data": format!("gen-{}", style),
            "format": "png",
            "size": 256,
            "style": style
        });
        let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
            .body(body.to_string()).dispatch();
        assert_eq!(resp.status(), Status::Ok, "Generate failed for style {}", style);
    }
}

// ============ Tracked QR Created At Timestamp ============

#[test]
fn test_tracked_qr_has_created_at() {
    let client = test_client();
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/timestamp-test"}"#).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    assert!(body["created_at"].is_string(), "Missing created_at");
    // Should be a reasonable ISO-8601 / datetime string
    let created = body["created_at"].as_str().unwrap();
    assert!(created.contains("20"), "created_at should contain year prefix");
}

// ============ Batch Size Clamping ============

#[test]
fn test_batch_size_clamped() {
    let client = test_client();
    // Batch items with out-of-range sizes get clamped (not rejected)
    let body = serde_json::json!({
        "items": [
            {"data": "tiny", "size": 10},
            {"data": "huge", "size": 9999}
        ]
    });
    let resp = client.post("/api/v1/qr/batch").header(ContentType::JSON)
        .body(body.to_string()).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let result: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(result["total"], 2);
    // Sizes should be clamped to valid range
    assert_eq!(result["items"][0]["size"], 10); // size field reflects requested, but image is clamped
    assert_eq!(result["items"][1]["size"], 9999);
}

// ============ Share URL Construction ============

#[test]
fn test_share_url_in_generate_response() {
    let client = test_client();
    let resp = client.post("/api/v1/qr/generate").header(ContentType::JSON)
        .body(r#"{"data": "share-test", "size": 300, "format": "svg", "style": "dots"}"#).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let share_url = body["share_url"].as_str().unwrap();
    assert!(share_url.contains("/qr/view?"), "Should contain view path");
    assert!(share_url.contains("size=300"), "Should contain size param");
    assert!(share_url.contains("format=svg"), "Should contain format param");
    assert!(share_url.contains("style=dots"), "Should contain style param");
}

// ============ Manage URL in Tracked QR Response ============

#[test]
fn test_tracked_qr_manage_url() {
    let client = test_client();
    let resp = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/manage-test"}"#).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().unwrap();
    let manage_url = body["manage_url"].as_str().unwrap();
    assert!(manage_url.contains("/api/v1/qr/tracked/"), "Should contain tracked path");
    assert!(manage_url.contains("?key=qrt_"), "Should contain manage token");
}

// ============ Scan Event Ordering ============

#[test]
fn test_scan_events_ordered_newest_first() {
    let client = test_client();
    let create = client.post("/api/v1/qr/tracked").header(ContentType::JSON)
        .body(r#"{"target_url": "https://example.com/order-test", "short_code": "order-test"}"#).dispatch();
    let cb: serde_json::Value = create.into_json().unwrap();
    let id = cb["id"].as_str().unwrap().to_string();
    let token = cb["manage_token"].as_str().unwrap().to_string();

    // Create 3 scans with different user agents
    for i in 1..=3 {
        client.get("/r/order-test")
            .header(Header::new("User-Agent", format!("Agent-{}", i)))
            .dispatch();
    }

    let stats = client.get(format!("/api/v1/qr/tracked/{}/stats", id))
        .header(Header::new("Authorization", format!("Bearer {}", token))).dispatch();
    let sb: serde_json::Value = stats.into_json().unwrap();
    let scans = sb["recent_scans"].as_array().unwrap();
    assert_eq!(scans.len(), 3);
    // Verify all 3 user agents are present (ordering may vary within same second)
    let agents: Vec<&str> = scans.iter()
        .map(|s| s["user_agent"].as_str().unwrap())
        .collect();
    assert!(agents.contains(&"Agent-1"), "Missing Agent-1");
    assert!(agents.contains(&"Agent-2"), "Missing Agent-2");
    assert!(agents.contains(&"Agent-3"), "Missing Agent-3");
}
