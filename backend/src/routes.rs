use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rocket::http::{ContentType, Status};
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::State;
use std::path::PathBuf;

use crate::auth::{ClientIp, ManageToken};
use crate::db::{hash_token, DbPool};
use crate::models::*;
use crate::qr;
use crate::rate_limit::RateLimiter;

// Default IP rate limit: 100 requests per window
const IP_RATE_LIMIT: u64 = 100;

/// Check IP rate limit and return error if exceeded.
fn check_ip_rate(ip: &ClientIp, limiter: &RateLimiter) -> Result<(), (Status, Json<ApiError>)> {
    let key = format!("ip:{}", ip.0);
    let result = limiter.check(&key, IP_RATE_LIMIT);
    if !result.allowed {
        return Err((
            Status::TooManyRequests,
            Json(ApiError {
                error: "Rate limit exceeded. Try again later.".to_string(),
                code: "RATE_LIMIT_EXCEEDED".to_string(),
                status: 429,
            }),
        ));
    }
    Ok(())
}

/// Build a stateless share URL that encodes QR params in the URL itself.
fn build_share_url(data: &str, size: u32, fg: &str, bg: &str, format: &str, style: &str) -> String {
    let base_url =
        std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let encoded_data = BASE64.encode(data.as_bytes());
    format!(
        "{}/qr/view?data={}&size={}&fg={}&bg={}&format={}&style={}",
        base_url.trim_end_matches('/'),
        urlencoding::encode(&encoded_data),
        size,
        urlencoding::encode(&fg.replace('#', "")),
        urlencoding::encode(&bg.replace('#', "")),
        format,
        style,
    )
}

// ============ Health & OpenAPI ============

#[get("/health")]
pub fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: 0,
    })
}

#[get("/openapi.json")]
pub fn openapi() -> (ContentType, &'static str) {
    (ContentType::JSON, include_str!("../openapi.json"))
}

#[get("/llms.txt")]
pub fn llms_txt() -> (ContentType, &'static str) {
    (ContentType::Text, include_str!("../llms.txt"))
}

/// Root-level /llms.txt for standard discovery (outside /api/v1)
#[get("/llms.txt", rank = 2)]
pub fn root_llms_txt() -> (ContentType, &'static str) {
    (ContentType::Text, include_str!("../llms.txt"))
}

// ============ QR Generation (Stateless, No Auth) ============

#[post("/qr/generate", format = "json", data = "<req>")]
pub fn generate_qr(
    req: Json<GenerateRequest>,
    ip: ClientIp,
    limiter: &State<RateLimiter>,
) -> Result<Json<QrResponse>, (Status, Json<ApiError>)> {
    check_ip_rate(&ip, limiter)?;
    let req = req.into_inner();

    if req.data.is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Data field cannot be empty".to_string(),
                code: "EMPTY_DATA".to_string(),
                status: 400,
            }),
        ));
    }

    if req.size < 64 || req.size > 4096 {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Size must be between 64 and 4096".to_string(),
                code: "INVALID_SIZE".to_string(),
                status: 400,
            }),
        ));
    }

    let fg_color = qr::parse_hex_color(&req.fg_color).map_err(|e| {
        (Status::BadRequest, Json(ApiError { error: e, code: "INVALID_FG_COLOR".to_string(), status: 400 }))
    })?;
    let bg_color = qr::parse_hex_color(&req.bg_color).map_err(|e| {
        (Status::BadRequest, Json(ApiError { error: e, code: "INVALID_BG_COLOR".to_string(), status: 400 }))
    })?;

    let options = qr::QrOptions {
        size: req.size,
        fg_color,
        bg_color,
        error_correction: qr::parse_ec_level(&req.error_correction),
        style: qr::QrStyle::parse(&req.style),
    };

    let (image_data, content_type) = match req.format.as_str() {
        "png" => {
            let data = qr::generate_png(&req.data, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError { error: e, code: "GENERATION_FAILED".to_string(), status: 500 }))
            })?;
            (data, "image/png")
        }
        "svg" => {
            let svg = qr::generate_svg(&req.data, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError { error: e, code: "GENERATION_FAILED".to_string(), status: 500 }))
            })?;
            (svg.into_bytes(), "image/svg+xml")
        }
        _ => {
            return Err((
                Status::BadRequest,
                Json(ApiError {
                    error: "Unsupported format. Use 'png' or 'svg'".to_string(),
                    code: "INVALID_FORMAT".to_string(),
                    status: 400,
                }),
            ));
        }
    };

    let image_base64 = format!("data:{};base64,{}", content_type, BASE64.encode(&image_data));
    let share_url = build_share_url(&req.data, req.size, &req.fg_color, &req.bg_color, &req.format, &req.style);

    Ok(Json(QrResponse {
        image_base64,
        share_url,
        format: req.format,
        size: req.size,
        data: req.data,
    }))
}

#[post("/qr/decode", data = "<data>")]
pub fn decode_qr(
    data: Vec<u8>,
    ip: ClientIp,
    limiter: &State<RateLimiter>,
) -> Result<Json<DecodeResponse>, (Status, Json<ApiError>)> {
    check_ip_rate(&ip, limiter)?;

    let img = image::load_from_memory(&data).map_err(|e| {
        (Status::BadRequest, Json(ApiError {
            error: format!("Failed to load image: {}", e),
            code: "INVALID_IMAGE".to_string(),
            status: 400,
        }))
    })?;

    let gray = img.to_luma8();
    let decoded = rqrr_decode(&gray);

    match decoded {
        Some(content) => Ok(Json(DecodeResponse {
            data: content,
            format: "qr".to_string(),
        })),
        None => Err((
            Status::UnprocessableEntity,
            Json(ApiError {
                error: "No QR code found in image".to_string(),
                code: "NO_QR_FOUND".to_string(),
                status: 422,
            }),
        )),
    }
}

fn rqrr_decode(img: &image::GrayImage) -> Option<String> {
    let mut prepared = rqrr::PreparedImage::prepare(img.clone());
    let grids = prepared.detect_grids();
    if let Some(grid) = grids.into_iter().next() {
        if let Ok((_meta, content)) = grid.decode() {
            return Some(content);
        }
    }
    None
}

#[post("/qr/batch", format = "json", data = "<req>")]
pub fn batch_generate(
    req: Json<BatchGenerateRequest>,
    ip: ClientIp,
    limiter: &State<RateLimiter>,
) -> Result<Json<BatchQrResponse>, (Status, Json<ApiError>)> {
    check_ip_rate(&ip, limiter)?;
    let req = req.into_inner();

    if req.items.is_empty() {
        return Err((Status::BadRequest, Json(ApiError {
            error: "Items array cannot be empty".to_string(), code: "EMPTY_BATCH".to_string(), status: 400,
        })));
    }
    if req.items.len() > 50 {
        return Err((Status::BadRequest, Json(ApiError {
            error: "Maximum 50 items per batch".to_string(), code: "BATCH_TOO_LARGE".to_string(), status: 400,
        })));
    }

    let mut responses = Vec::new();
    for item in &req.items {
        let fg_color = qr::parse_hex_color(&item.fg_color).unwrap_or([0, 0, 0, 255]);
        let bg_color = qr::parse_hex_color(&item.bg_color).unwrap_or([255, 255, 255, 255]);

        let options = qr::QrOptions {
            size: item.size.clamp(64, 4096),
            fg_color,
            bg_color,
            error_correction: qr::parse_ec_level(&item.error_correction),
            style: qr::QrStyle::parse(&item.style),
        };

        let (image_data, content_type) = match item.format.as_str() {
            "svg" => match qr::generate_svg(&item.data, &options) {
                Ok(svg) => (svg.into_bytes(), "image/svg+xml"),
                Err(_) => continue,
            },
            _ => match qr::generate_png(&item.data, &options) {
                Ok(data) => (data, "image/png"),
                Err(_) => continue,
            },
        };

        let image_base64 = format!("data:{};base64,{}", content_type, BASE64.encode(&image_data));
        let share_url = build_share_url(&item.data, item.size, &item.fg_color, &item.bg_color, &item.format, &item.style);

        responses.push(QrResponse {
            image_base64,
            share_url,
            format: item.format.clone(),
            size: item.size,
            data: item.data.clone(),
        });
    }

    let total = responses.len();
    Ok(Json(BatchQrResponse { items: responses, total }))
}

#[post("/qr/template/<template_type>", format = "json", data = "<body>")]
pub fn generate_from_template(
    template_type: &str,
    body: Json<serde_json::Value>,
    ip: ClientIp,
    limiter: &State<RateLimiter>,
) -> Result<Json<QrResponse>, (Status, Json<ApiError>)> {
    check_ip_rate(&ip, limiter)?;
    let body = body.into_inner();

    let (data, format, size) = match template_type {
        "wifi" => {
            let ssid = body.get("ssid").and_then(|v| v.as_str()).ok_or_else(|| {
                (Status::BadRequest, Json(ApiError { error: "Missing 'ssid' field".to_string(), code: "MISSING_FIELD".to_string(), status: 400 }))
            })?;
            let password = body.get("password").and_then(|v| v.as_str()).unwrap_or("");
            let encryption = body.get("encryption").and_then(|v| v.as_str()).unwrap_or("WPA2");
            let hidden = body.get("hidden").and_then(|v| v.as_bool()).unwrap_or(false);
            let format = body.get("format").and_then(|v| v.as_str()).unwrap_or("png").to_string();
            let size = body.get("size").and_then(|v| v.as_u64()).unwrap_or(256) as u32;
            (qr::wifi_data(ssid, password, encryption, hidden), format, size)
        }
        "vcard" => {
            let name = body.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                (Status::BadRequest, Json(ApiError { error: "Missing 'name' field".to_string(), code: "MISSING_FIELD".to_string(), status: 400 }))
            })?;
            let format = body.get("format").and_then(|v| v.as_str()).unwrap_or("png").to_string();
            let size = body.get("size").and_then(|v| v.as_u64()).unwrap_or(256) as u32;
            let data = qr::vcard_data(
                name,
                body.get("email").and_then(|v| v.as_str()),
                body.get("phone").and_then(|v| v.as_str()),
                body.get("org").and_then(|v| v.as_str()),
                body.get("title").and_then(|v| v.as_str()),
                body.get("url").and_then(|v| v.as_str()),
            );
            (data, format, size)
        }
        "url" => {
            let mut url = body.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                (Status::BadRequest, Json(ApiError { error: "Missing 'url' field".to_string(), code: "MISSING_FIELD".to_string(), status: 400 }))
            })?.to_string();

            let mut params = Vec::new();
            if let Some(source) = body.get("utm_source").and_then(|v| v.as_str()) {
                params.push(format!("utm_source={}", source));
            }
            if let Some(medium) = body.get("utm_medium").and_then(|v| v.as_str()) {
                params.push(format!("utm_medium={}", medium));
            }
            if let Some(campaign) = body.get("utm_campaign").and_then(|v| v.as_str()) {
                params.push(format!("utm_campaign={}", campaign));
            }
            if !params.is_empty() {
                let separator = if url.contains('?') { "&" } else { "?" };
                url = format!("{}{}{}", url, separator, params.join("&"));
            }

            let format = body.get("format").and_then(|v| v.as_str()).unwrap_or("png").to_string();
            let size = body.get("size").and_then(|v| v.as_u64()).unwrap_or(256) as u32;
            (url, format, size)
        }
        _ => {
            return Err((Status::BadRequest, Json(ApiError {
                error: format!("Unknown template type: '{}'. Available: wifi, vcard, url", template_type),
                code: "UNKNOWN_TEMPLATE".to_string(), status: 400,
            })));
        }
    };

    let style_str = body.get("style").and_then(|v| v.as_str()).unwrap_or("square");
    let options = qr::QrOptions {
        size: size.clamp(64, 4096),
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qr::parse_ec_level("M"),
        style: qr::QrStyle::parse(style_str),
    };

    let (image_data, content_type) = match format.as_str() {
        "svg" => {
            let svg = qr::generate_svg(&data, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError { error: e, code: "GENERATION_FAILED".to_string(), status: 500 }))
            })?;
            (svg.into_bytes(), "image/svg+xml")
        }
        _ => {
            let png = qr::generate_png(&data, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError { error: e, code: "GENERATION_FAILED".to_string(), status: 500 }))
            })?;
            (png, "image/png")
        }
    };

    let image_base64 = format!("data:{};base64,{}", content_type, BASE64.encode(&image_data));
    let share_url = build_share_url(&data, size, "#000000", "#FFFFFF", &format, style_str);

    Ok(Json(QrResponse {
        image_base64,
        share_url,
        format,
        size,
        data,
    }))
}

// ============ Share URL View (Stateless) ============

/// Renders a QR code from URL params — no database needed.
/// GET /qr/view?data=<base64>&size=300&fg=000000&bg=ffffff&format=png&style=square
#[get("/qr/view?<data>&<size>&<fg>&<bg>&<format>&<style>")]
pub fn view_qr(
    data: &str,
    size: Option<u32>,
    fg: Option<&str>,
    bg: Option<&str>,
    format: Option<&str>,
    style: Option<&str>,
) -> Result<(ContentType, Vec<u8>), (Status, Json<ApiError>)> {
    let decoded_data = BASE64.decode(data).map_err(|_| {
        (Status::BadRequest, Json(ApiError {
            error: "Invalid base64 data".to_string(), code: "INVALID_DATA".to_string(), status: 400,
        }))
    })?;
    let content = String::from_utf8(decoded_data).map_err(|_| {
        (Status::BadRequest, Json(ApiError {
            error: "Invalid UTF-8 data".to_string(), code: "INVALID_DATA".to_string(), status: 400,
        }))
    })?;

    let size = size.unwrap_or(256).clamp(64, 4096);
    let fg_hex = format!("#{}", fg.unwrap_or("000000"));
    let bg_hex = format!("#{}", bg.unwrap_or("FFFFFF"));
    let fmt = format.unwrap_or("png");
    let style_str = style.unwrap_or("square");

    let fg_color = qr::parse_hex_color(&fg_hex).unwrap_or([0, 0, 0, 255]);
    let bg_color = qr::parse_hex_color(&bg_hex).unwrap_or([255, 255, 255, 255]);

    let options = qr::QrOptions {
        size,
        fg_color,
        bg_color,
        error_correction: qr::parse_ec_level("M"),
        style: qr::QrStyle::parse(style_str),
    };

    match fmt {
        "svg" => {
            let svg = qr::generate_svg(&content, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError { error: e, code: "GENERATION_FAILED".to_string(), status: 500 }))
            })?;
            Ok((ContentType::SVG, svg.into_bytes()))
        }
        _ => {
            let png = qr::generate_png(&content, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError { error: e, code: "GENERATION_FAILED".to_string(), status: 500 }))
            })?;
            Ok((ContentType::PNG, png))
        }
    }
}

// ============ Tracked QR / Short URLs (Per-Resource Token Auth) ============

/// Rate limit for tracked QR creation (per IP)
const TRACKED_CREATE_RATE_LIMIT: u64 = 20;

#[post("/qr/tracked", format = "json", data = "<req>")]
pub fn create_tracked_qr(
    req: Json<CreateTrackedQrRequest>,
    ip: ClientIp,
    limiter: &State<RateLimiter>,
    db: &State<DbPool>,
) -> Result<Json<TrackedQrResponse>, (Status, Json<ApiError>)> {
    // IP-based rate limit for creation
    let key = format!("ip:tracked:{}", ip.0);
    let result = limiter.check(&key, TRACKED_CREATE_RATE_LIMIT);
    if !result.allowed {
        return Err((Status::TooManyRequests, Json(ApiError {
            error: "Rate limit exceeded for tracked QR creation. Try again later.".to_string(),
            code: "RATE_LIMIT_EXCEEDED".to_string(), status: 429,
        })));
    }

    let req = req.into_inner();

    if req.target_url.is_empty() {
        return Err((Status::BadRequest, Json(ApiError {
            error: "target_url cannot be empty".to_string(), code: "EMPTY_TARGET_URL".to_string(), status: 400,
        })));
    }
    if !req.target_url.starts_with("http://") && !req.target_url.starts_with("https://") {
        return Err((Status::BadRequest, Json(ApiError {
            error: "target_url must start with http:// or https://".to_string(), code: "INVALID_URL".to_string(), status: 400,
        })));
    }

    let short_code = match req.short_code {
        Some(ref code) => {
            if code.len() < 3 || code.len() > 32 {
                return Err((Status::BadRequest, Json(ApiError {
                    error: "short_code must be 3-32 characters".to_string(), code: "INVALID_SHORT_CODE".to_string(), status: 400,
                })));
            }
            if !code.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                return Err((Status::BadRequest, Json(ApiError {
                    error: "short_code must be alphanumeric, hyphens, or underscores".to_string(),
                    code: "INVALID_SHORT_CODE".to_string(), status: 400,
                })));
            }
            code.clone()
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string().replace("-", "");
            id[..8].to_string()
        }
    };

    {
        let conn = db.lock().unwrap();
        let exists: bool = conn
            .query_row("SELECT COUNT(*) > 0 FROM tracked_qr WHERE short_code = ?1", rusqlite::params![short_code], |row| row.get(0))
            .unwrap_or(false);
        if exists {
            return Err((Status::Conflict, Json(ApiError {
                error: format!("Short code '{}' is already taken", short_code),
                code: "SHORT_CODE_TAKEN".to_string(), status: 409,
            })));
        }
    }

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let short_url = format!("{}/r/{}", base_url.trim_end_matches('/'), short_code);

    let fg_color = qr::parse_hex_color(&req.fg_color).map_err(|e| {
        (Status::BadRequest, Json(ApiError { error: e, code: "INVALID_FG_COLOR".to_string(), status: 400 }))
    })?;
    let bg_color = qr::parse_hex_color(&req.bg_color).map_err(|e| {
        (Status::BadRequest, Json(ApiError { error: e, code: "INVALID_BG_COLOR".to_string(), status: 400 }))
    })?;

    let options = qr::QrOptions {
        size: req.size.clamp(64, 4096),
        fg_color,
        bg_color,
        error_correction: qr::parse_ec_level(&req.error_correction),
        style: qr::QrStyle::parse(&req.style),
    };

    let (image_data, content_type) = match req.format.as_str() {
        "svg" => {
            let svg = qr::generate_svg(&short_url, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError { error: e, code: "GENERATION_FAILED".to_string(), status: 500 }))
            })?;
            (svg.into_bytes(), "image/svg+xml")
        }
        _ => {
            let png = qr::generate_png(&short_url, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError { error: e, code: "GENERATION_FAILED".to_string(), status: 500 }))
            })?;
            (png, "image/png")
        }
    };

    let qr_id = uuid::Uuid::new_v4().to_string();
    let tracked_id = uuid::Uuid::new_v4().to_string();
    let manage_token = format!("qrt_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
    let manage_token_hash_val = hash_token(&manage_token);
    let image_base64 = format!("data:{};base64,{}", content_type, BASE64.encode(&image_data));

    let conn = db.lock().unwrap();

    conn.execute(
        "INSERT INTO qr_codes (id, data, format, size, fg_color, bg_color, error_correction, style, image_data) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![qr_id, short_url, req.format, req.size, req.fg_color, req.bg_color, req.error_correction, req.style, image_data],
    ).map_err(|e| {
        (Status::InternalServerError, Json(ApiError { error: format!("Failed to store QR code: {}", e), code: "DB_ERROR".to_string(), status: 500 }))
    })?;

    conn.execute(
        "INSERT INTO tracked_qr (id, qr_id, short_code, target_url, manage_token_hash, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![tracked_id, qr_id, short_code, req.target_url, manage_token_hash_val, req.expires_at],
    ).map_err(|e| {
        (Status::InternalServerError, Json(ApiError { error: format!("Failed to create tracked QR: {}", e), code: "DB_ERROR".to_string(), status: 500 }))
    })?;

    let created_at = conn
        .query_row("SELECT created_at FROM tracked_qr WHERE id = ?1", rusqlite::params![tracked_id], |row| row.get::<_, String>(0))
        .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());

    let manage_url = format!("{}/api/v1/qr/tracked/{}?key={}", base_url.trim_end_matches('/'), tracked_id, manage_token);

    Ok(Json(TrackedQrResponse {
        id: tracked_id,
        qr_id: qr_id.clone(),
        short_code,
        short_url: short_url.clone(),
        target_url: req.target_url,
        manage_token,
        manage_url,
        scan_count: 0,
        expires_at: req.expires_at,
        created_at,
        qr: QrResponse {
            image_base64,
            share_url: short_url,
            format: req.format,
            size: req.size,
            data: "tracked".to_string(),
        },
    }))
}

#[get("/qr/tracked/<id>/stats")]
pub fn get_tracked_qr_stats(
    id: &str,
    token: ManageToken,
    db: &State<DbPool>,
) -> Result<Json<TrackedQrStatsResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_token(&token.0);

    let tracked = conn.query_row(
        "SELECT id, short_code, target_url, scan_count, expires_at, created_at 
         FROM tracked_qr WHERE id = ?1 AND manage_token_hash = ?2",
        rusqlite::params![id, token_hash],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                   row.get::<_, i64>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, String>(5)?)),
    ).map_err(|_| {
        (Status::NotFound, Json(ApiError { error: "Tracked QR code not found or invalid token".to_string(), code: "NOT_FOUND".to_string(), status: 404 }))
    })?;

    let mut stmt = conn.prepare(
        "SELECT id, scanned_at, user_agent, referrer FROM scan_events WHERE tracked_qr_id = ?1 ORDER BY scanned_at DESC LIMIT 100",
    ).map_err(|e| {
        (Status::InternalServerError, Json(ApiError { error: format!("Database error: {}", e), code: "DB_ERROR".to_string(), status: 500 }))
    })?;

    let recent_scans = stmt.query_map(rusqlite::params![id], |row| {
        Ok(ScanEventResponse { id: row.get(0)?, scanned_at: row.get(1)?, user_agent: row.get(2)?, referrer: row.get(3)? })
    }).map_err(|e| {
        (Status::InternalServerError, Json(ApiError { error: format!("Query error: {}", e), code: "DB_ERROR".to_string(), status: 500 }))
    })?.filter_map(|r| r.ok()).collect();

    Ok(Json(TrackedQrStatsResponse {
        id: tracked.0, short_code: tracked.1, target_url: tracked.2,
        scan_count: tracked.3, expires_at: tracked.4, created_at: tracked.5, recent_scans,
    }))
}

#[delete("/qr/tracked/<id>")]
pub fn delete_tracked_qr(
    id: &str,
    token: ManageToken,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let token_hash = hash_token(&token.0);

    let qr_id: String = conn.query_row(
        "SELECT qr_id FROM tracked_qr WHERE id = ?1 AND manage_token_hash = ?2",
        rusqlite::params![id, token_hash], |row| row.get(0),
    ).map_err(|_| {
        (Status::NotFound, Json(ApiError { error: "Tracked QR code not found or invalid token".to_string(), code: "NOT_FOUND".to_string(), status: 404 }))
    })?;

    conn.execute("DELETE FROM scan_events WHERE tracked_qr_id = ?1", rusqlite::params![id]).unwrap_or(0);
    conn.execute("DELETE FROM tracked_qr WHERE id = ?1", rusqlite::params![id]).unwrap_or(0);
    conn.execute("DELETE FROM qr_codes WHERE id = ?1", rusqlite::params![qr_id]).unwrap_or(0);

    Ok(Json(serde_json::json!({"deleted": true, "id": id})))
}

// ============ Short URL Redirect ============

pub struct ScanMeta {
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

#[rocket::async_trait]
impl<'r> rocket::request::FromRequest<'r> for ScanMeta {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r rocket::Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        let user_agent = request.headers().get_one("User-Agent").map(|s| s.to_string());
        let referrer = request.headers().get_one("Referer").map(|s| s.to_string());
        rocket::request::Outcome::Success(ScanMeta { user_agent, referrer })
    }
}

#[get("/r/<code>")]
pub fn redirect_short_url(
    code: &str,
    db: &State<DbPool>,
    meta: ScanMeta,
) -> Result<Redirect, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();

    let result = conn.query_row(
        "SELECT id, target_url, expires_at FROM tracked_qr WHERE short_code = ?1",
        rusqlite::params![code],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?)),
    );

    match result {
        Ok((tracked_id, target_url, expires_at)) => {
            if let Some(ref exp) = expires_at {
                let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                if now > *exp {
                    return Err((Status::Gone, Json(ApiError {
                        error: "This short URL has expired".to_string(), code: "EXPIRED".to_string(), status: 410,
                    })));
                }
            }

            let scan_id = uuid::Uuid::new_v4().to_string();
            let _ = conn.execute(
                "INSERT INTO scan_events (id, tracked_qr_id, user_agent, referrer) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![scan_id, tracked_id, meta.user_agent, meta.referrer],
            );
            let _ = conn.execute("UPDATE tracked_qr SET scan_count = scan_count + 1 WHERE id = ?1", rusqlite::params![tracked_id]);

            Ok(Redirect::temporary(target_url))
        }
        Err(_) => Err((Status::NotFound, Json(ApiError {
            error: "Short URL not found".to_string(), code: "NOT_FOUND".to_string(), status: 404,
        }))),
    }
}

// ============ SPA Fallback ============

#[get("/<_path..>", rank = 20)]
pub fn spa_fallback(_path: PathBuf) -> Option<(ContentType, Vec<u8>)> {
    let static_dir: PathBuf = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));
    let index_path = static_dir.join("index.html");
    std::fs::read(&index_path).ok().map(|bytes| (ContentType::HTML, bytes))
}
