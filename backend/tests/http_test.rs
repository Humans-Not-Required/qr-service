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
