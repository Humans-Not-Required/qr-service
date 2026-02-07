use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rocket::http::{ContentType, Status};
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::State;
use std::path::PathBuf;

use crate::auth::AuthenticatedKey;
use crate::db::{hash_key, DbPool};
use crate::models::*;
use crate::qr;

// ============ Health & OpenAPI ============

#[get("/health")]
pub fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: 0, // TODO: track actual uptime
    })
}

#[get("/openapi.json")]
pub fn openapi() -> (ContentType, &'static str) {
    (ContentType::JSON, include_str!("../openapi.json"))
}

// ============ QR Generation ============

#[post("/qr/generate", format = "json", data = "<req>")]
pub fn generate_qr(
    req: Json<GenerateRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<QrResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();

    // Validate
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
        (
            Status::BadRequest,
            Json(ApiError {
                error: e,
                code: "INVALID_FG_COLOR".to_string(),
                status: 400,
            }),
        )
    })?;

    let bg_color = qr::parse_hex_color(&req.bg_color).map_err(|e| {
        (
            Status::BadRequest,
            Json(ApiError {
                error: e,
                code: "INVALID_BG_COLOR".to_string(),
                status: 400,
            }),
        )
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
                (
                    Status::InternalServerError,
                    Json(ApiError {
                        error: e,
                        code: "GENERATION_FAILED".to_string(),
                        status: 500,
                    }),
                )
            })?;
            (data, "image/png")
        }
        "svg" => {
            let svg = qr::generate_svg(&req.data, &options).map_err(|e| {
                (
                    Status::InternalServerError,
                    Json(ApiError {
                        error: e,
                        code: "GENERATION_FAILED".to_string(),
                        status: 500,
                    }),
                )
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

    let id = uuid::Uuid::new_v4().to_string();
    let image_base64 = format!(
        "data:{};base64,{}",
        content_type,
        BASE64.encode(&image_data)
    );

    // Store in database
    let conn = db.lock().unwrap();
    let _ = conn.execute(
        "INSERT INTO qr_codes (id, api_key_id, data, format, size, fg_color, bg_color, error_correction, style, image_data) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            id,
            key.id,
            req.data,
            req.format,
            req.size,
            req.fg_color,
            req.bg_color,
            req.error_correction,
            req.style,
            image_data,
        ],
    );

    let created_at = conn
        .query_row(
            "SELECT created_at FROM qr_codes WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());

    Ok(Json(QrResponse {
        id,
        data: req.data,
        format: req.format,
        size: req.size,
        image_base64,
        created_at,
    }))
}

#[post("/qr/decode", data = "<data>")]
pub fn decode_qr(
    data: Vec<u8>,
    _key: AuthenticatedKey,
) -> Result<Json<DecodeResponse>, (Status, Json<ApiError>)> {
    // Try to decode the image
    let img = image::load_from_memory(&data).map_err(|e| {
        (
            Status::BadRequest,
            Json(ApiError {
                error: format!("Failed to load image: {}", e),
                code: "INVALID_IMAGE".to_string(),
                status: 400,
            }),
        )
    })?;

    let gray = img.to_luma8();

    // Use a simple decoder approach
    // For production, we'd use a proper QR decoder like rqrr
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
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<BatchQrResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();

    if req.items.is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Items array cannot be empty".to_string(),
                code: "EMPTY_BATCH".to_string(),
                status: 400,
            }),
        ));
    }

    if req.items.len() > 50 {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "Maximum 50 items per batch".to_string(),
                code: "BATCH_TOO_LARGE".to_string(),
                status: 400,
            }),
        ));
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

        let id = uuid::Uuid::new_v4().to_string();
        let image_base64 = format!(
            "data:{};base64,{}",
            content_type,
            BASE64.encode(&image_data)
        );

        // Store in db
        let conn = db.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO qr_codes (id, api_key_id, data, format, size, fg_color, bg_color, error_correction, style, image_data) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![id, key.id, item.data, item.format, item.size, item.fg_color, item.bg_color, item.error_correction, item.style, image_data],
        );

        responses.push(QrResponse {
            id,
            data: item.data.clone(),
            format: item.format.clone(),
            size: item.size,
            image_base64,
            created_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    let total = responses.len();
    Ok(Json(BatchQrResponse {
        items: responses,
        total,
    }))
}

#[post("/qr/template/<template_type>", format = "json", data = "<body>")]
pub fn generate_from_template(
    template_type: &str,
    body: Json<serde_json::Value>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<QrResponse>, (Status, Json<ApiError>)> {
    let body = body.into_inner();

    let (data, format, size) = match template_type {
        "wifi" => {
            let ssid = body.get("ssid").and_then(|v| v.as_str()).ok_or_else(|| {
                (
                    Status::BadRequest,
                    Json(ApiError {
                        error: "Missing 'ssid' field".to_string(),
                        code: "MISSING_FIELD".to_string(),
                        status: 400,
                    }),
                )
            })?;
            let password = body.get("password").and_then(|v| v.as_str()).unwrap_or("");
            let encryption = body
                .get("encryption")
                .and_then(|v| v.as_str())
                .unwrap_or("WPA2");
            let hidden = body
                .get("hidden")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let format = body
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("png")
                .to_string();
            let size = body.get("size").and_then(|v| v.as_u64()).unwrap_or(256) as u32;

            (
                qr::wifi_data(ssid, password, encryption, hidden),
                format,
                size,
            )
        }
        "vcard" => {
            let name = body.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                (
                    Status::BadRequest,
                    Json(ApiError {
                        error: "Missing 'name' field".to_string(),
                        code: "MISSING_FIELD".to_string(),
                        status: 400,
                    }),
                )
            })?;
            let format = body
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("png")
                .to_string();
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
            let mut url = body
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    (
                        Status::BadRequest,
                        Json(ApiError {
                            error: "Missing 'url' field".to_string(),
                            code: "MISSING_FIELD".to_string(),
                            status: 400,
                        }),
                    )
                })?
                .to_string();

            // Add UTM parameters if provided
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

            let format = body
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("png")
                .to_string();
            let size = body.get("size").and_then(|v| v.as_u64()).unwrap_or(256) as u32;
            (url, format, size)
        }
        _ => {
            return Err((
                Status::BadRequest,
                Json(ApiError {
                    error: format!(
                        "Unknown template type: '{}'. Available: wifi, vcard, url",
                        template_type
                    ),
                    code: "UNKNOWN_TEMPLATE".to_string(),
                    status: 400,
                }),
            ));
        }
    };

    // Generate the QR code
    let style_str = body
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("square");
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
                (
                    Status::InternalServerError,
                    Json(ApiError {
                        error: e,
                        code: "GENERATION_FAILED".to_string(),
                        status: 500,
                    }),
                )
            })?;
            (svg.into_bytes(), "image/svg+xml")
        }
        _ => {
            let png = qr::generate_png(&data, &options).map_err(|e| {
                (
                    Status::InternalServerError,
                    Json(ApiError {
                        error: e,
                        code: "GENERATION_FAILED".to_string(),
                        status: 500,
                    }),
                )
            })?;
            (png, "image/png")
        }
    };

    let id = uuid::Uuid::new_v4().to_string();
    let image_base64 = format!(
        "data:{};base64,{}",
        content_type,
        BASE64.encode(&image_data)
    );

    let conn = db.lock().unwrap();
    let _ = conn.execute(
        "INSERT INTO qr_codes (id, api_key_id, data, format, size, template, image_data) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, key.id, data, format, size, template_type, image_data],
    );

    Ok(Json(QrResponse {
        id,
        data,
        format,
        size,
        image_base64,
        created_at: chrono::Utc::now().to_rfc3339(),
    }))
}

// ============ History ============

#[get("/qr/history?<page>&<per_page>")]
pub fn get_history(
    page: Option<usize>,
    per_page: Option<usize>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<HistoryResponse>, (Status, Json<ApiError>)> {
    let page = page.unwrap_or(1).max(1);
    let per_page = per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let conn = db.lock().unwrap();

    let total: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM qr_codes WHERE api_key_id = ?1",
            rusqlite::params![key.id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT id, data, format, size, created_at FROM qr_codes WHERE api_key_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
    ).map_err(|e| {
        (Status::InternalServerError, Json(ApiError {
            error: format!("Database error: {}", e),
            code: "DB_ERROR".to_string(),
            status: 500,
        }))
    })?;

    let items = stmt
        .query_map(
            rusqlite::params![key.id, per_page as i64, offset as i64],
            |row| {
                Ok(QrHistoryItem {
                    id: row.get(0)?,
                    data: row.get(1)?,
                    format: row.get(2)?,
                    size: row.get::<_, i64>(3)? as u32,
                    created_at: row.get(4)?,
                })
            },
        )
        .map_err(|e| {
            (
                Status::InternalServerError,
                Json(ApiError {
                    error: format!("Query error: {}", e),
                    code: "DB_ERROR".to_string(),
                    status: 500,
                }),
            )
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(HistoryResponse {
        items,
        total,
        page,
        per_page,
    }))
}

#[get("/qr/<id>")]
pub fn get_qr_by_id(
    id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<QrResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();

    conn.query_row(
        "SELECT id, data, format, size, image_data, created_at FROM qr_codes WHERE id = ?1 AND api_key_id = ?2",
        rusqlite::params![id, key.id],
        |row| {
            let image_data: Vec<u8> = row.get(4)?;
            let format: String = row.get(2)?;
            let content_type = if format == "svg" { "image/svg+xml" } else { "image/png" };
            Ok(QrResponse {
                id: row.get(0)?,
                data: row.get(1)?,
                format,
                size: row.get::<_, i64>(3)? as u32,
                image_base64: format!("data:{};base64,{}", content_type, BASE64.encode(&image_data)),
                created_at: row.get(5)?,
            })
        },
    ).map(Json).map_err(|_| {
        (Status::NotFound, Json(ApiError {
            error: "QR code not found".to_string(),
            code: "NOT_FOUND".to_string(),
            status: 404,
        }))
    })
}

/// Returns the raw image bytes (PNG or SVG) with proper Content-Type header.
/// Agents can fetch this directly to get the image without base64 overhead.
#[get("/qr/<id>/image")]
pub fn get_qr_image(
    id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<(ContentType, Vec<u8>), (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();

    conn.query_row(
        "SELECT format, image_data FROM qr_codes WHERE id = ?1 AND api_key_id = ?2",
        rusqlite::params![id, key.id],
        |row| {
            let format: String = row.get(0)?;
            let image_data: Vec<u8> = row.get(1)?;
            Ok((format, image_data))
        },
    )
    .map(|(format, data)| {
        let ct = if format == "svg" {
            ContentType::SVG
        } else {
            ContentType::PNG
        };
        (ct, data)
    })
    .map_err(|_| {
        (
            Status::NotFound,
            Json(ApiError {
                error: "QR code not found".to_string(),
                code: "NOT_FOUND".to_string(),
                status: 404,
            }),
        )
    })
}

#[delete("/qr/<id>")]
pub fn delete_qr(
    id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let affected = conn
        .execute(
            "DELETE FROM qr_codes WHERE id = ?1 AND api_key_id = ?2",
            rusqlite::params![id, key.id],
        )
        .unwrap_or(0);

    if affected > 0 {
        Ok(Json(serde_json::json!({"deleted": true, "id": id})))
    } else {
        Err((
            Status::NotFound,
            Json(ApiError {
                error: "QR code not found".to_string(),
                code: "NOT_FOUND".to_string(),
                status: 404,
            }),
        ))
    }
}

// ============ Tracked QR / Short URLs ============

/// Create a tracked QR code that wraps a short URL for scan analytics.
#[post("/qr/tracked", format = "json", data = "<req>")]
pub fn create_tracked_qr(
    req: Json<CreateTrackedQrRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<TrackedQrResponse>, (Status, Json<ApiError>)> {
    let req = req.into_inner();

    if req.target_url.is_empty() {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "target_url cannot be empty".to_string(),
                code: "EMPTY_TARGET_URL".to_string(),
                status: 400,
            }),
        ));
    }

    // Validate URL format (basic check)
    if !req.target_url.starts_with("http://") && !req.target_url.starts_with("https://") {
        return Err((
            Status::BadRequest,
            Json(ApiError {
                error: "target_url must start with http:// or https://".to_string(),
                code: "INVALID_URL".to_string(),
                status: 400,
            }),
        ));
    }

    // Generate or validate short code
    let short_code = match req.short_code {
        Some(ref code) => {
            if code.len() < 3 || code.len() > 32 {
                return Err((
                    Status::BadRequest,
                    Json(ApiError {
                        error: "short_code must be 3-32 characters".to_string(),
                        code: "INVALID_SHORT_CODE".to_string(),
                        status: 400,
                    }),
                ));
            }
            if !code
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Err((
                    Status::BadRequest,
                    Json(ApiError {
                        error: "short_code must be alphanumeric, hyphens, or underscores"
                            .to_string(),
                        code: "INVALID_SHORT_CODE".to_string(),
                        status: 400,
                    }),
                ));
            }
            code.clone()
        }
        None => {
            // Generate a random 8-char code
            let id = uuid::Uuid::new_v4().to_string().replace("-", "");
            id[..8].to_string()
        }
    };

    // Check uniqueness
    {
        let conn = db.lock().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM tracked_qr WHERE short_code = ?1",
                rusqlite::params![short_code],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if exists {
            return Err((
                Status::Conflict,
                Json(ApiError {
                    error: format!("Short code '{}' is already taken", short_code),
                    code: "SHORT_CODE_TAKEN".to_string(),
                    status: 409,
                }),
            ));
        }
    }

    // Build the short URL that the QR code will encode.
    // The service itself serves the redirect at /r/<short_code>.
    // In production, ROCKET_ADDRESS + ROCKET_PORT determine the base URL.
    // We use a configurable base or fall back to a sensible default.
    let base_url =
        std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let short_url = format!("{}/r/{}", base_url.trim_end_matches('/'), short_code);

    // Generate the QR code encoding the short URL
    let fg_color = qr::parse_hex_color(&req.fg_color).map_err(|e| {
        (
            Status::BadRequest,
            Json(ApiError {
                error: e,
                code: "INVALID_FG_COLOR".to_string(),
                status: 400,
            }),
        )
    })?;
    let bg_color = qr::parse_hex_color(&req.bg_color).map_err(|e| {
        (
            Status::BadRequest,
            Json(ApiError {
                error: e,
                code: "INVALID_BG_COLOR".to_string(),
                status: 400,
            }),
        )
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
                (
                    Status::InternalServerError,
                    Json(ApiError {
                        error: e,
                        code: "GENERATION_FAILED".to_string(),
                        status: 500,
                    }),
                )
            })?;
            (svg.into_bytes(), "image/svg+xml")
        }
        _ => {
            let png = qr::generate_png(&short_url, &options).map_err(|e| {
                (
                    Status::InternalServerError,
                    Json(ApiError {
                        error: e,
                        code: "GENERATION_FAILED".to_string(),
                        status: 500,
                    }),
                )
            })?;
            (png, "image/png")
        }
    };

    let qr_id = uuid::Uuid::new_v4().to_string();
    let tracked_id = uuid::Uuid::new_v4().to_string();
    let image_base64 = format!(
        "data:{};base64,{}",
        content_type,
        BASE64.encode(&image_data)
    );

    let conn = db.lock().unwrap();

    // Insert QR code record
    conn.execute(
        "INSERT INTO qr_codes (id, api_key_id, data, format, size, fg_color, bg_color, error_correction, style, image_data) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            qr_id,
            key.id,
            short_url,
            req.format,
            req.size,
            req.fg_color,
            req.bg_color,
            req.error_correction,
            req.style,
            image_data,
        ],
    ).map_err(|e| {
        (
            Status::InternalServerError,
            Json(ApiError {
                error: format!("Failed to store QR code: {}", e),
                code: "DB_ERROR".to_string(),
                status: 500,
            }),
        )
    })?;

    // Insert tracked QR record
    conn.execute(
        "INSERT INTO tracked_qr (id, qr_id, short_code, target_url, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![tracked_id, qr_id, short_code, req.target_url, req.expires_at],
    ).map_err(|e| {
        (
            Status::InternalServerError,
            Json(ApiError {
                error: format!("Failed to create tracked QR: {}", e),
                code: "DB_ERROR".to_string(),
                status: 500,
            }),
        )
    })?;

    let created_at = conn
        .query_row(
            "SELECT created_at FROM tracked_qr WHERE id = ?1",
            rusqlite::params![tracked_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());

    Ok(Json(TrackedQrResponse {
        id: tracked_id,
        qr_id: qr_id.clone(),
        short_code: short_code.clone(),
        short_url: short_url.clone(),
        target_url: req.target_url,
        scan_count: 0,
        expires_at: req.expires_at,
        created_at: created_at.clone(),
        qr: QrResponse {
            id: qr_id,
            data: short_url,
            format: req.format,
            size: req.size,
            image_base64,
            created_at,
        },
    }))
}

/// List all tracked QR codes for the authenticated user.
#[get("/qr/tracked?<page>&<per_page>")]
pub fn list_tracked_qr(
    page: Option<usize>,
    per_page: Option<usize>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<TrackedQrListResponse>, (Status, Json<ApiError>)> {
    let page = page.unwrap_or(1).max(1);
    let per_page = per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let conn = db.lock().unwrap();

    let total: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM tracked_qr t JOIN qr_codes q ON t.qr_id = q.id WHERE q.api_key_id = ?1",
            rusqlite::params![key.id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.short_code, t.target_url, t.scan_count, t.expires_at, t.created_at 
         FROM tracked_qr t JOIN qr_codes q ON t.qr_id = q.id 
         WHERE q.api_key_id = ?1 
         ORDER BY t.created_at DESC LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| {
            (
                Status::InternalServerError,
                Json(ApiError {
                    error: format!("Database error: {}", e),
                    code: "DB_ERROR".to_string(),
                    status: 500,
                }),
            )
        })?;

    let items = stmt
        .query_map(
            rusqlite::params![key.id, per_page as i64, offset as i64],
            |row| {
                Ok(TrackedQrListItem {
                    id: row.get(0)?,
                    short_code: row.get(1)?,
                    target_url: row.get(2)?,
                    scan_count: row.get(3)?,
                    expires_at: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|e| {
            (
                Status::InternalServerError,
                Json(ApiError {
                    error: format!("Query error: {}", e),
                    code: "DB_ERROR".to_string(),
                    status: 500,
                }),
            )
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(TrackedQrListResponse { items, total }))
}

/// Get scan analytics for a tracked QR code.
#[get("/qr/tracked/<id>/stats")]
pub fn get_tracked_qr_stats(
    id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<TrackedQrStatsResponse>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();

    // Verify ownership via the linked qr_codes record
    let tracked = conn
        .query_row(
            "SELECT t.id, t.short_code, t.target_url, t.scan_count, t.expires_at, t.created_at 
         FROM tracked_qr t JOIN qr_codes q ON t.qr_id = q.id 
         WHERE t.id = ?1 AND q.api_key_id = ?2",
            rusqlite::params![id, key.id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .map_err(|_| {
            (
                Status::NotFound,
                Json(ApiError {
                    error: "Tracked QR code not found".to_string(),
                    code: "NOT_FOUND".to_string(),
                    status: 404,
                }),
            )
        })?;

    // Get recent scan events (last 100)
    let mut stmt = conn
        .prepare(
            "SELECT id, scanned_at, user_agent, referrer FROM scan_events 
         WHERE tracked_qr_id = ?1 ORDER BY scanned_at DESC LIMIT 100",
        )
        .map_err(|e| {
            (
                Status::InternalServerError,
                Json(ApiError {
                    error: format!("Database error: {}", e),
                    code: "DB_ERROR".to_string(),
                    status: 500,
                }),
            )
        })?;

    let recent_scans = stmt
        .query_map(rusqlite::params![id], |row| {
            Ok(ScanEventResponse {
                id: row.get(0)?,
                scanned_at: row.get(1)?,
                user_agent: row.get(2)?,
                referrer: row.get(3)?,
            })
        })
        .map_err(|e| {
            (
                Status::InternalServerError,
                Json(ApiError {
                    error: format!("Query error: {}", e),
                    code: "DB_ERROR".to_string(),
                    status: 500,
                }),
            )
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(TrackedQrStatsResponse {
        id: tracked.0,
        short_code: tracked.1,
        target_url: tracked.2,
        scan_count: tracked.3,
        expires_at: tracked.4,
        created_at: tracked.5,
        recent_scans,
    }))
}

/// Delete a tracked QR code (and its scan events).
#[delete("/qr/tracked/<id>")]
pub fn delete_tracked_qr(
    id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();

    // Verify ownership
    let qr_id: String = conn.query_row(
        "SELECT t.qr_id FROM tracked_qr t JOIN qr_codes q ON t.qr_id = q.id WHERE t.id = ?1 AND q.api_key_id = ?2",
        rusqlite::params![id, key.id],
        |row| row.get(0),
    ).map_err(|_| {
        (Status::NotFound, Json(ApiError {
            error: "Tracked QR code not found".to_string(),
            code: "NOT_FOUND".to_string(),
            status: 404,
        }))
    })?;

    // Delete scan events first (FK constraint)
    conn.execute(
        "DELETE FROM scan_events WHERE tracked_qr_id = ?1",
        rusqlite::params![id],
    )
    .unwrap_or(0);

    // Delete tracked record
    conn.execute(
        "DELETE FROM tracked_qr WHERE id = ?1",
        rusqlite::params![id],
    )
    .unwrap_or(0);

    // Also delete the underlying QR code
    conn.execute(
        "DELETE FROM qr_codes WHERE id = ?1",
        rusqlite::params![qr_id],
    )
    .unwrap_or(0);

    Ok(Json(serde_json::json!({"deleted": true, "id": id})))
}

// ============ API Keys ============

#[get("/keys")]
pub fn list_keys(
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<KeyResponse>>, (Status, Json<ApiError>)> {
    if !key.is_admin {
        return Err((
            Status::Forbidden,
            Json(ApiError {
                error: "Admin access required".to_string(),
                code: "FORBIDDEN".to_string(),
                status: 403,
            }),
        ));
    }

    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, last_used_at, requests_count, rate_limit, active FROM api_keys ORDER BY created_at DESC"
    ).map_err(|e| {
        (Status::InternalServerError, Json(ApiError {
            error: format!("Database error: {}", e),
            code: "DB_ERROR".to_string(),
            status: 500,
        }))
    })?;

    let keys = stmt
        .query_map([], |row| {
            Ok(KeyResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                key: None,
                created_at: row.get(2)?,
                last_used_at: row.get(3)?,
                requests_count: row.get(4)?,
                rate_limit: row.get(5)?,
                active: row.get::<_, i32>(6)? == 1,
            })
        })
        .map_err(|e| {
            (
                Status::InternalServerError,
                Json(ApiError {
                    error: format!("Query error: {}", e),
                    code: "DB_ERROR".to_string(),
                    status: 500,
                }),
            )
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(keys))
}

#[post("/keys", format = "json", data = "<req>")]
pub fn create_key(
    req: Json<CreateKeyRequest>,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<KeyResponse>, (Status, Json<ApiError>)> {
    if !key.is_admin {
        return Err((
            Status::Forbidden,
            Json(ApiError {
                error: "Admin access required".to_string(),
                code: "FORBIDDEN".to_string(),
                status: 403,
            }),
        ));
    }

    let req = req.into_inner();
    let new_key = format!("qrs_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
    let key_hash_val = hash_key(&new_key);
    let id = uuid::Uuid::new_v4().to_string();

    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO api_keys (id, name, key_hash, rate_limit) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, req.name, key_hash_val, req.rate_limit],
    )
    .map_err(|e| {
        (
            Status::InternalServerError,
            Json(ApiError {
                error: format!("Failed to create key: {}", e),
                code: "DB_ERROR".to_string(),
                status: 500,
            }),
        )
    })?;

    Ok(Json(KeyResponse {
        id,
        name: req.name,
        key: Some(new_key),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_used_at: None,
        requests_count: 0,
        rate_limit: req.rate_limit,
        active: true,
    }))
}

#[delete("/keys/<id>")]
pub fn delete_key(
    id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    if !key.is_admin {
        return Err((
            Status::Forbidden,
            Json(ApiError {
                error: "Admin access required".to_string(),
                code: "FORBIDDEN".to_string(),
                status: 403,
            }),
        ));
    }

    let conn = db.lock().unwrap();
    let affected = conn
        .execute(
            "UPDATE api_keys SET active = 0 WHERE id = ?1",
            rusqlite::params![id],
        )
        .unwrap_or(0);

    if affected > 0 {
        Ok(Json(serde_json::json!({"revoked": true, "id": id})))
    } else {
        Err((
            Status::NotFound,
            Json(ApiError {
                error: "API key not found".to_string(),
                code: "NOT_FOUND".to_string(),
                status: 404,
            }),
        ))
    }
}

// ============ Short URL Redirect (mounted at root, not /api/v1) ============

/// Captures optional scan metadata from request headers.
pub struct ScanMeta {
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

#[rocket::async_trait]
impl<'r> rocket::request::FromRequest<'r> for ScanMeta {
    type Error = std::convert::Infallible;

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user_agent = request
            .headers()
            .get_one("User-Agent")
            .map(|s| s.to_string());
        let referrer = request.headers().get_one("Referer").map(|s| s.to_string());
        rocket::request::Outcome::Success(ScanMeta {
            user_agent,
            referrer,
        })
    }
}

/// Redirect handler for tracked QR short URLs.
/// When someone scans a tracked QR code, they hit /r/<code> which redirects
/// to the target URL while recording the scan event.
#[get("/r/<code>")]
pub fn redirect_short_url(
    code: &str,
    db: &State<DbPool>,
    meta: ScanMeta,
) -> Result<Redirect, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();

    // Look up the tracked QR by short code
    let result = conn.query_row(
        "SELECT id, target_url, expires_at FROM tracked_qr WHERE short_code = ?1",
        rusqlite::params![code],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        },
    );

    match result {
        Ok((tracked_id, target_url, expires_at)) => {
            // Check expiry
            if let Some(ref exp) = expires_at {
                let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                if now > *exp {
                    return Err((
                        Status::Gone,
                        Json(ApiError {
                            error: "This short URL has expired".to_string(),
                            code: "EXPIRED".to_string(),
                            status: 410,
                        }),
                    ));
                }
            }

            // Record scan event
            let scan_id = uuid::Uuid::new_v4().to_string();
            let _ = conn.execute(
                "INSERT INTO scan_events (id, tracked_qr_id, user_agent, referrer) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![scan_id, tracked_id, meta.user_agent, meta.referrer],
            );

            // Increment scan count
            let _ = conn.execute(
                "UPDATE tracked_qr SET scan_count = scan_count + 1 WHERE id = ?1",
                rusqlite::params![tracked_id],
            );

            Ok(Redirect::temporary(target_url))
        }
        Err(_) => Err((
            Status::NotFound,
            Json(ApiError {
                error: "Short URL not found".to_string(),
                code: "NOT_FOUND".to_string(),
                status: 404,
            }),
        )),
    }
}

// ============ SPA Fallback ============

/// Catch-all route for client-side routing. Serves index.html for any GET
/// request that didn't match an API route, static file, or short URL redirect.
/// Rank 20 ensures this runs after FileServer and all other routes.
#[get("/<_path..>", rank = 20)]
pub fn spa_fallback(_path: PathBuf) -> Option<(ContentType, Vec<u8>)> {
    let static_dir: PathBuf = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));
    let index_path = static_dir.join("index.html");
    std::fs::read(&index_path)
        .ok()
        .map(|bytes| (ContentType::HTML, bytes))
}
