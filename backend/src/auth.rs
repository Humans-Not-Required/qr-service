use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};

/// Extracts a manage token from Bearer header, X-API-Key header, or ?key= query param.
/// Used for tracked QR management routes (stats, delete).
#[derive(Debug)]
pub struct ManageToken(pub String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ManageToken {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Try Authorization: Bearer <token>
        if let Some(auth) = request.headers().get_one("Authorization") {
            if let Some(token) = auth.strip_prefix("Bearer ") {
                return Outcome::Success(ManageToken(token.to_string()));
            }
            return Outcome::Error((
                Status::Unauthorized,
                "Invalid authorization format. Use: Bearer YOUR_TOKEN",
            ));
        }

        // Try X-API-Key header
        if let Some(token) = request.headers().get_one("X-API-Key") {
            return Outcome::Success(ManageToken(token.to_string()));
        }

        // Try ?key= query param
        if let Some(query) = request.uri().query() {
            for seg in query.segments() {
                if seg.0 == "key" {
                    return Outcome::Success(ManageToken(seg.1.to_string()));
                }
            }
        }

        Outcome::Error((
            Status::Unauthorized,
            "Missing manage token. Use Authorization: Bearer TOKEN, X-API-Key header, or ?key= query param",
        ))
    }
}

/// Extracts the client IP for IP-based rate limiting on public routes.
#[derive(Debug)]
pub struct ClientIp(pub String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientIp {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
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
