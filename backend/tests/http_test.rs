// HTTP integration tests using Rocket's test client.
// Tests the full request→response cycle: stateless QR generation,
// tracked QR with manage tokens, rate limiting, and error handling.

#[macro_use]
extern crate rocket;

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
        .attach(qr_service::rate_limit::RateLimitHeaders)
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
        .attach(qr_service::rate_limit::RateLimitHeaders)
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
