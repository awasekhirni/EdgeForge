// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Token-Bucket Rate Limiting Middleware
//!
//! ## Purpose
//! Per-IP token-bucket rate limiting. Limits the number of requests per minute
//! per client IP address. Uses an in-memory token bucket algorithm.
//!
//! ## What This File Does
//! - `RateLimiter` — thread-safe token-bucket rate limiter
//! - `RateLimiter::new()` — create with capacity and refill rate
//! - `RateLimiter::check()` — check if a request is allowed
//! - `rate_limit_middleware()` — axum middleware function
//!
//! ## Dependencies
//! - `axum` — middleware extraction
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `parking_lot` — thread-safe state
//! - `std::time::Instant` — timing
//!
//! ## Output
//! Passes the request through if allowed, returns 429 if rate limited.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - rate_limit.rs - function - STATUS`

use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Token bucket for a single client IP.
#[derive(Clone, Debug)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: f64) -> Self {
        Self {
            tokens: capacity,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self, capacity: f64, refill_per_second: f64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * refill_per_second).min(capacity);
        self.last_refill = now;
    }

    fn try_consume(&mut self, cost: f64) -> bool {
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }
}

/// Thread-safe per-IP token-bucket rate limiter.
///
/// ## Purpose
/// Limits the number of requests per minute per client IP. Uses the
/// token-bucket algorithm: tokens are refilled at a constant rate, and
/// each request consumes one token.
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, TokenBucket>>,
    capacity: f64,
    refill_per_second: f64,
}

impl RateLimiter {
    /// Creates a new rate limiter.
    ///
    /// ## Parameters
    /// - `capacity`: Maximum number of tokens (burst capacity).
    /// - `refill_per_minute`: Number of tokens refilled per minute.
    ///
    /// ## Returns
    /// A new `RateLimiter` instance.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-api - rate_limit.rs - RateLimiter::new - STATUS`
    pub fn new(capacity: u32, refill_per_minute: u32) -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "rate_limit.rs", "RateLimiter::new");
        Self {
            buckets: Mutex::new(HashMap::new()),
            capacity: capacity as f64,
            refill_per_second: refill_per_minute as f64 / 60.0,
        }
    }

    /// Check if a request from the given IP is allowed.
    ///
    /// ## Purpose
    /// Consumes one token from the IP's bucket. If no tokens are available,
    /// the request is rate-limited.
    ///
    /// ## Parameters
    /// - `ip`: The client IP address string.
    ///
    /// ## Returns
    /// `true` if the request is allowed, `false` if rate-limited.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-api - rate_limit.rs - check - STATUS`
    pub fn check(&self, ip: &str) -> bool {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "rate_limit.rs", "check");
        let mut buckets = self.buckets.lock();
        let bucket = buckets.entry(ip.to_string()).or_insert_with(|| TokenBucket::new(self.capacity));
        bucket.refill(self.capacity, self.refill_per_second);
        let allowed = bucket.try_consume(1.0);
        if !allowed {
            guard.error("rate limited");
        } else {
            guard.success();
        }
        allowed
    }

    /// Returns the current number of tokens for an IP.
    ///
    /// ## Parameters
    /// - `ip`: The client IP address string.
    ///
    /// ## Returns
    /// The number of available tokens (0.0 if IP not tracked).
    pub fn available_tokens(&self, ip: &str) -> f64 {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "rate_limit.rs", "available_tokens");
        let mut buckets = self.buckets.lock();
        let bucket = buckets.entry(ip.to_string()).or_insert_with(|| TokenBucket::new(self.capacity));
        bucket.refill(self.capacity, self.refill_per_second);
        bucket.tokens
    }

    /// Clears all rate-limiting state.
    pub fn clear(&self) {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "rate_limit.rs", "clear");
        self.buckets.lock().clear();
    }
}

/// Axum middleware: enforce per-IP rate limiting.
///
/// ## Purpose
/// Checks the client's IP against the rate limiter. If the rate limit is
/// exceeded, returns 429 Too Many Requests.
///
/// ## Parameters
/// - `limiter`: The rate limiter instance.
/// - `req`: The incoming request.
/// - `next`: The next middleware/handler in the chain.
///
/// ## Returns
/// The response from the next handler, or 429 if rate-limited.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - rate_limit.rs - rate_limit_middleware - STATUS`
pub async fn rate_limit_middleware(
    limiter: Arc<RateLimiter>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "rate_limit.rs", "rate_limit_middleware");
    let ip = addr.ip().to_string();
    if !limiter.check(&ip) {
        guard.error(&format!("rate limited for IP {ip}"));
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    guard.success();
    next.run(req).await
}

use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(10, 100);
        for _ in 0..10 {
            assert!(limiter.check("127.0.0.1"));
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(2, 0); // 0 refill = no new tokens
        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.1"));
        assert!(!limiter.check("127.0.0.1")); // 3rd request blocked
    }

    #[test]
    fn test_rate_limiter_separate_ips() {
        let limiter = RateLimiter::new(1, 0);
        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.2")); // different IP, allowed
        assert!(!limiter.check("127.0.0.1")); // same IP, blocked
    }

    #[test]
    fn test_rate_limiter_refill() {
        let limiter = RateLimiter::new(1, 10000); // very fast refill
        assert!(limiter.check("127.0.0.1"));
        assert!(!limiter.check("127.0.0.1")); // immediately: no refill yet
        std::thread::sleep(Duration::from_millis(10));
        assert!(limiter.check("127.0.0.1")); // refilled
    }

    #[test]
    fn test_rate_limiter_available_tokens() {
        let limiter = RateLimiter::new(5, 0);
        limiter.check("127.0.0.1");
        limiter.check("127.0.0.1");
        let tokens = limiter.available_tokens("127.0.0.1");
        assert!((tokens - 3.0).abs() < 0.1);
    }

    #[test]
    fn test_rate_limiter_clear() {
        let limiter = RateLimiter::new(1, 0);
        limiter.check("127.0.0.1");
        assert!(!limiter.check("127.0.0.1"));
        limiter.clear();
        assert!(limiter.check("127.0.0.1")); // cleared, allowed again
    }
}
