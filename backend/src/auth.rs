use crate::db::{hash_key, DbPool};
use crate::rate_limit::RateLimiter;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;

/// Authenticated API key (kept for tracked QR and admin routes).
#[derive(Debug)]
pub struct AuthenticatedKey {
    pub id: String,
    #[allow(dead_code)]
    pub name: String,
    pub is_admin: bool,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedKey {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let db = match request.guard::<&State<DbPool>>().await {
            Outcome::Success(db) => db,
            _ => return Outcome::Error((Status::InternalServerError, "Database unavailable")),
        };

        let limiter = match request.guard::<&State<RateLimiter>>().await {
            Outcome::Success(l) => l,
            _ => return Outcome::Error((Status::InternalServerError, "Rate limiter unavailable")),
        };

        let key = match request.headers().get_one("Authorization") {
            Some(auth) => {
                if let Some(key) = auth.strip_prefix("Bearer ") {
                    key.to_string()
                } else {
                    return Outcome::Error((
                        Status::Unauthorized,
                        "Invalid authorization format. Use: Bearer YOUR_API_KEY",
                    ));
                }
            }
            None => match request.headers().get_one("X-API-Key") {
                Some(key) => key.to_string(),
                None => {
                    return Outcome::Error((
                        Status::Unauthorized,
                        "Missing API key. Use Authorization: Bearer YOUR_KEY or X-API-Key header",
                    ))
                }
            },
        };

        let key_hash = hash_key(&key);
        let conn = db.lock().unwrap();

        match conn.query_row(
            "SELECT id, name, is_admin, rate_limit FROM api_keys WHERE key_hash = ?1 AND active = 1",
            rusqlite::params![key_hash],
            |row| {
                Ok((
                    AuthenticatedKey {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        is_admin: row.get::<_, i32>(2)? == 1,
                    },
                    row.get::<_, i64>(3)?,
                ))
            },
        ) {
            Ok((auth_key, rate_limit)) => {
                let _ = conn.execute(
                    "UPDATE api_keys SET last_used_at = datetime('now'), requests_count = requests_count + 1 WHERE id = ?1",
                    rusqlite::params![auth_key.id],
                );
                drop(conn);

                let result = limiter.check(&auth_key.id, rate_limit as u64);
                let _ = request.local_cache(|| Some(result.clone()));

                if !result.allowed {
                    return Outcome::Error((
                        Status::TooManyRequests,
                        "Rate limit exceeded. Try again later.",
                    ));
                }

                Outcome::Success(auth_key)
            }
            Err(_) => Outcome::Error((Status::Unauthorized, "Invalid API key")),
        }
    }
}

/// Extracts the client IP for IP-based rate limiting on public routes.
#[derive(Debug)]
pub struct ClientIp(pub String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientIp {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Try X-Forwarded-For, X-Real-Ip, then socket addr
        let ip = request
            .headers()
            .get_one("X-Forwarded-For")
            .and_then(|v| v.split(',').next())
            .map(|s| s.trim().to_string())
            .or_else(|| {
                request
                    .headers()
                    .get_one("X-Real-Ip")
                    .map(|s| s.to_string())
            })
            .or_else(|| request.remote().map(|a| a.ip().to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        Outcome::Success(ClientIp(ip))
    }
}
