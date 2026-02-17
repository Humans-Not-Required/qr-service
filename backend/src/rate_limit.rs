use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rocket::http::Header;
use rocket::response::Responder;
use rocket::Request;

/// Fixed-window rate limiter.
///
/// Each key (e.g. IP address) gets a counter that resets every `window` duration.
/// Callers pass in the per-key limit when checking.
pub struct RateLimiter {
    window: Duration,
    /// key_id → (window_start, count)
    buckets: Mutex<HashMap<String, (Instant, u64)>>,
}

/// Result of a rate limit check.
/// Stored in request-local state so the response fairing can attach headers.
#[derive(Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Configured limit for this key.
    pub limit: u64,
    /// Requests remaining in the current window.
    pub remaining: u64,
    /// Seconds until the current window resets.
    pub reset_secs: u64,
}

/// Wrapper responder that attaches rate limit headers to any inner response.
///
/// Use this instead of the fairing when you have the `RateLimitResult` available
/// in the route handler (e.g. from `check_ip_rate`).
pub struct RateLimited<T> {
    pub inner: T,
    pub rate_limit: RateLimitResult,
}

impl<'r, 'o: 'r, T: Responder<'r, 'o>> Responder<'r, 'o> for RateLimited<T> {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        let mut response = self.inner.respond_to(request)?;
        response.set_header(Header::new(
            "X-RateLimit-Limit",
            self.rate_limit.limit.to_string(),
        ));
        response.set_header(Header::new(
            "X-RateLimit-Remaining",
            self.rate_limit.remaining.to_string(),
        ));
        response.set_header(Header::new(
            "X-RateLimit-Reset",
            self.rate_limit.reset_secs.to_string(),
        ));
        Ok(response)
    }
}

impl RateLimiter {
    /// Create a new rate limiter with the given window duration.
    pub fn new(window: Duration) -> Self {
        RateLimiter {
            window,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Check (and consume) one request for `key_id` with the given `limit`.
    ///
    /// Returns a `RateLimitResult` indicating whether the request is allowed
    /// and the current rate limit state for response headers.
    pub fn check(&self, key_id: &str, limit: u64) -> RateLimitResult {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());

        let entry = buckets
            .entry(key_id.to_string())
            .or_insert_with(|| (now, 0));

        // If the window has elapsed, reset.
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }

        let reset_secs = self
            .window
            .checked_sub(now.duration_since(entry.0))
            .unwrap_or(Duration::ZERO)
            .as_secs();

        if entry.1 >= limit {
            RateLimitResult {
                allowed: false,
                limit,
                remaining: 0,
                reset_secs,
            }
        } else {
            entry.1 += 1;
            RateLimitResult {
                allowed: true,
                limit,
                remaining: limit.saturating_sub(entry.1),
                reset_secs,
            }
        }
    }

    /// Periodically prune stale entries to prevent unbounded memory growth.
    /// Call this from a background task or on a timer.
    #[allow(dead_code)]
    pub fn prune_stale(&self) {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        buckets.retain(|_, (start, _)| now.duration_since(*start) < self.window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_under_limit() {
        let rl = RateLimiter::new(Duration::from_secs(60));
        let r = rl.check("key1", 10);
        assert!(r.allowed);
        assert_eq!(r.remaining, 9);
        assert_eq!(r.limit, 10);
    }

    #[test]
    fn blocks_at_limit() {
        let rl = RateLimiter::new(Duration::from_secs(60));
        for _ in 0..5 {
            rl.check("key1", 5);
        }
        let r = rl.check("key1", 5);
        assert!(!r.allowed);
        assert_eq!(r.remaining, 0);
    }

    #[test]
    fn separate_keys_independent() {
        let rl = RateLimiter::new(Duration::from_secs(60));
        for _ in 0..5 {
            rl.check("key1", 5);
        }
        // key1 is exhausted
        assert!(!rl.check("key1", 5).allowed);
        // key2 should still be fine
        assert!(rl.check("key2", 5).allowed);
    }
}
