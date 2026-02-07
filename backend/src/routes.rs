use rocket::serde::json::Json;
use rocket::http::{ContentType, Status};
use rocket::State;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::auth::AuthenticatedKey;
use crate::db::{DbPool, hash_key};
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
        return Err((Status::BadRequest, Json(ApiError {
            error: "Data field cannot be empty".to_string(),
            code: "EMPTY_DATA".to_string(),
            status: 400,
        })));
    }
    
    if req.size < 64 || req.size > 4096 {
        return Err((Status::BadRequest, Json(ApiError {
            error: "Size must be between 64 and 4096".to_string(),
            code: "INVALID_SIZE".to_string(),
            status: 400,
        })));
    }
    
    let fg_color = qr::parse_hex_color(&req.fg_color).map_err(|e| {
        (Status::BadRequest, Json(ApiError {
            error: e,
            code: "INVALID_FG_COLOR".to_string(),
            status: 400,
        }))
    })?;
    
    let bg_color = qr::parse_hex_color(&req.bg_color).map_err(|e| {
        (Status::BadRequest, Json(ApiError {
            error: e,
            code: "INVALID_BG_COLOR".to_string(),
            status: 400,
        }))
    })?;
    
    let options = qr::QrOptions {
        size: req.size,
        fg_color,
        bg_color,
        error_correction: qr::parse_ec_level(&req.error_correction),
    };
    
    let (image_data, content_type) = match req.format.as_str() {
        "png" => {
            let data = qr::generate_png(&req.data, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError {
                    error: e,
                    code: "GENERATION_FAILED".to_string(),
                    status: 500,
                }))
            })?;
            (data, "image/png")
        }
        "svg" => {
            let svg = qr::generate_svg(&req.data, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError {
                    error: e,
                    code: "GENERATION_FAILED".to_string(),
                    status: 500,
                }))
            })?;
            (svg.into_bytes(), "image/svg+xml")
        }
        _ => {
            return Err((Status::BadRequest, Json(ApiError {
                error: "Unsupported format. Use 'png' or 'svg'".to_string(),
                code: "INVALID_FORMAT".to_string(),
                status: 400,
            })));
        }
    };
    
    let id = uuid::Uuid::new_v4().to_string();
    let image_base64 = format!("data:{};base64,{}", content_type, BASE64.encode(&image_data));
    
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
    
    let created_at = conn.query_row(
        "SELECT created_at FROM qr_codes WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get::<_, String>(0),
    ).unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
    
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
        (Status::BadRequest, Json(ApiError {
            error: format!("Failed to load image: {}", e),
            code: "INVALID_IMAGE".to_string(),
            status: 400,
        }))
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
        None => Err((Status::UnprocessableEntity, Json(ApiError {
            error: "No QR code found in image".to_string(),
            code: "NO_QR_FOUND".to_string(),
            status: 422,
        }))),
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
        return Err((Status::BadRequest, Json(ApiError {
            error: "Items array cannot be empty".to_string(),
            code: "EMPTY_BATCH".to_string(),
            status: 400,
        })));
    }
    
    if req.items.len() > 50 {
        return Err((Status::BadRequest, Json(ApiError {
            error: "Maximum 50 items per batch".to_string(),
            code: "BATCH_TOO_LARGE".to_string(),
            status: 400,
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
        };
        
        let (image_data, content_type) = match item.format.as_str() {
            "svg" => {
                match qr::generate_svg(&item.data, &options) {
                    Ok(svg) => (svg.into_bytes(), "image/svg+xml"),
                    Err(_) => continue,
                }
            }
            _ => {
                match qr::generate_png(&item.data, &options) {
                    Ok(data) => (data, "image/png"),
                    Err(_) => continue,
                }
            }
        };
        
        let id = uuid::Uuid::new_v4().to_string();
        let image_base64 = format!("data:{};base64,{}", content_type, BASE64.encode(&image_data));
        
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
                (Status::BadRequest, Json(ApiError {
                    error: "Missing 'ssid' field".to_string(),
                    code: "MISSING_FIELD".to_string(),
                    status: 400,
                }))
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
                (Status::BadRequest, Json(ApiError {
                    error: "Missing 'name' field".to_string(),
                    code: "MISSING_FIELD".to_string(),
                    status: 400,
                }))
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
                (Status::BadRequest, Json(ApiError {
                    error: "Missing 'url' field".to_string(),
                    code: "MISSING_FIELD".to_string(),
                    status: 400,
                }))
            })?.to_string();
            
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
            
            let format = body.get("format").and_then(|v| v.as_str()).unwrap_or("png").to_string();
            let size = body.get("size").and_then(|v| v.as_u64()).unwrap_or(256) as u32;
            (url, format, size)
        }
        _ => {
            return Err((Status::BadRequest, Json(ApiError {
                error: format!("Unknown template type: '{}'. Available: wifi, vcard, url", template_type),
                code: "UNKNOWN_TEMPLATE".to_string(),
                status: 400,
            })));
        }
    };
    
    // Generate the QR code
    let options = qr::QrOptions {
        size: size.clamp(64, 4096),
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qr::parse_ec_level("M"),
    };
    
    let (image_data, content_type) = match format.as_str() {
        "svg" => {
            let svg = qr::generate_svg(&data, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError {
                    error: e,
                    code: "GENERATION_FAILED".to_string(),
                    status: 500,
                }))
            })?;
            (svg.into_bytes(), "image/svg+xml")
        }
        _ => {
            let png = qr::generate_png(&data, &options).map_err(|e| {
                (Status::InternalServerError, Json(ApiError {
                    error: e,
                    code: "GENERATION_FAILED".to_string(),
                    status: 500,
                }))
            })?;
            (png, "image/png")
        }
    };
    
    let id = uuid::Uuid::new_v4().to_string();
    let image_base64 = format!("data:{};base64,{}", content_type, BASE64.encode(&image_data));
    
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
    
    let total: usize = conn.query_row(
        "SELECT COUNT(*) FROM qr_codes WHERE api_key_id = ?1",
        rusqlite::params![key.id],
        |row| row.get(0),
    ).unwrap_or(0);
    
    let mut stmt = conn.prepare(
        "SELECT id, data, format, size, created_at FROM qr_codes WHERE api_key_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
    ).map_err(|e| {
        (Status::InternalServerError, Json(ApiError {
            error: format!("Database error: {}", e),
            code: "DB_ERROR".to_string(),
            status: 500,
        }))
    })?;
    
    let items = stmt.query_map(
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
    ).map_err(|e| {
        (Status::InternalServerError, Json(ApiError {
            error: format!("Query error: {}", e),
            code: "DB_ERROR".to_string(),
            status: 500,
        }))
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

#[delete("/qr/<id>")]
pub fn delete_qr(
    id: &str,
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<serde_json::Value>, (Status, Json<ApiError>)> {
    let conn = db.lock().unwrap();
    let affected = conn.execute(
        "DELETE FROM qr_codes WHERE id = ?1 AND api_key_id = ?2",
        rusqlite::params![id, key.id],
    ).unwrap_or(0);
    
    if affected > 0 {
        Ok(Json(serde_json::json!({"deleted": true, "id": id})))
    } else {
        Err((Status::NotFound, Json(ApiError {
            error: "QR code not found".to_string(),
            code: "NOT_FOUND".to_string(),
            status: 404,
        })))
    }
}

// ============ API Keys ============

#[get("/keys")]
pub fn list_keys(
    key: AuthenticatedKey,
    db: &State<DbPool>,
) -> Result<Json<Vec<KeyResponse>>, (Status, Json<ApiError>)> {
    if !key.is_admin {
        return Err((Status::Forbidden, Json(ApiError {
            error: "Admin access required".to_string(),
            code: "FORBIDDEN".to_string(),
            status: 403,
        })));
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
    
    let keys = stmt.query_map([], |row| {
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
    }).map_err(|e| {
        (Status::InternalServerError, Json(ApiError {
            error: format!("Query error: {}", e),
            code: "DB_ERROR".to_string(),
            status: 500,
        }))
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
        return Err((Status::Forbidden, Json(ApiError {
            error: "Admin access required".to_string(),
            code: "FORBIDDEN".to_string(),
            status: 403,
        })));
    }
    
    let req = req.into_inner();
    let new_key = format!("qrs_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
    let key_hash_val = hash_key(&new_key);
    let id = uuid::Uuid::new_v4().to_string();
    
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO api_keys (id, name, key_hash, rate_limit) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, req.name, key_hash_val, req.rate_limit],
    ).map_err(|e| {
        (Status::InternalServerError, Json(ApiError {
            error: format!("Failed to create key: {}", e),
            code: "DB_ERROR".to_string(),
            status: 500,
        }))
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
        return Err((Status::Forbidden, Json(ApiError {
            error: "Admin access required".to_string(),
            code: "FORBIDDEN".to_string(),
            status: 403,
        })));
    }
    
    let conn = db.lock().unwrap();
    let affected = conn.execute(
        "UPDATE api_keys SET active = 0 WHERE id = ?1",
        rusqlite::params![id],
    ).unwrap_or(0);
    
    if affected > 0 {
        Ok(Json(serde_json::json!({"revoked": true, "id": id})))
    } else {
        Err((Status::NotFound, Json(ApiError {
            error: "API key not found".to_string(),
            code: "NOT_FOUND".to_string(),
            status: 404,
        })))
    }
}
