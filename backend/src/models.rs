use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub data: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(default = "default_fg_color")]
    pub fg_color: String,
    #[serde(default = "default_bg_color")]
    pub bg_color: String,
    #[serde(default = "default_error_correction")]
    pub error_correction: String,
    #[serde(default = "default_style")]
    pub style: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchGenerateRequest {
    pub items: Vec<GenerateRequest>,
}

// Typed template structs — kept for future migration from serde_json::Value
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct WifiTemplateRequest {
    pub ssid: String,
    pub password: String,
    #[serde(default = "default_wifi_encryption")]
    pub encryption: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_size")]
    pub size: u32,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct VCardTemplateRequest {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub org: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_size")]
    pub size: u32,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct UrlTemplateRequest {
    pub url: String,
    #[serde(default)]
    pub utm_source: Option<String>,
    #[serde(default)]
    pub utm_medium: Option<String>,
    #[serde(default)]
    pub utm_campaign: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_size")]
    pub size: u32,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TemplateRequest {
    #[serde(rename = "wifi")]
    Wifi(WifiTemplateRequest),
    #[serde(rename = "vcard")]
    VCard(VCardTemplateRequest),
    #[serde(rename = "url")]
    Url(UrlTemplateRequest),
}

#[derive(Debug, Serialize)]
pub struct QrResponse {
    pub image_base64: String,
    pub share_url: String,
    pub format: String,
    pub size: u32,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct BatchQrResponse {
    pub items: Vec<QrResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct DecodeResponse {
    pub data: String,
    pub format: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    #[serde(default = "default_rate_limit")]
    pub rate_limit: i64,
}

#[derive(Debug, Serialize)]
pub struct KeyResponse {
    pub id: String,
    pub name: String,
    pub key: Option<String>, // Only returned on creation
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub requests_count: i64,
    pub rate_limit: i64,
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
    pub status: u16,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
}

fn default_format() -> String {
    "png".to_string()
}
fn default_size() -> u32 {
    256
}
fn default_fg_color() -> String {
    "#000000".to_string()
}
fn default_bg_color() -> String {
    "#FFFFFF".to_string()
}
fn default_error_correction() -> String {
    "M".to_string()
}
fn default_style() -> String {
    "square".to_string()
}
#[allow(dead_code)]
fn default_wifi_encryption() -> String {
    "WPA2".to_string()
}
fn default_rate_limit() -> i64 {
    100
}

// ============ Tracked QR / Short URLs ============

#[derive(Debug, Deserialize)]
pub struct CreateTrackedQrRequest {
    pub target_url: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(default = "default_fg_color")]
    pub fg_color: String,
    #[serde(default = "default_bg_color")]
    pub bg_color: String,
    #[serde(default = "default_error_correction")]
    pub error_correction: String,
    #[serde(default = "default_style")]
    pub style: String,
    /// Optional custom short code (auto-generated if omitted)
    pub short_code: Option<String>,
    /// Optional expiry as ISO-8601 timestamp
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TrackedQrResponse {
    pub id: String,
    pub qr_id: String,
    pub short_code: String,
    pub short_url: String,
    pub target_url: String,
    pub scan_count: i64,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub qr: QrResponse,
}

#[derive(Debug, Serialize)]
pub struct ScanEventResponse {
    pub id: String,
    pub scanned_at: String,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TrackedQrStatsResponse {
    pub id: String,
    pub short_code: String,
    pub target_url: String,
    pub scan_count: i64,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub recent_scans: Vec<ScanEventResponse>,
}

#[derive(Debug, Serialize)]
pub struct TrackedQrListItem {
    pub id: String,
    pub short_code: String,
    pub target_url: String,
    pub scan_count: i64,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct TrackedQrListResponse {
    pub items: Vec<TrackedQrListItem>,
    pub total: usize,
}
