//! Self-contained per-client token-bucket rate limiter.
//!
//! The server runs behind a Cloudflare tunnel, so every request's socket peer is Cloudflare, not
//! the real client. We therefore key on the `CF-Connecting-IP` header (the real client IP that
//! Cloudflare injects), falling back to `X-Forwarded-For` and finally the socket peer address.
//!
//! Limits are deliberately generous: ordinary anisette use is a handful of requests per sign-in or
//! download, so a legitimate user (even several sharing one NAT/CGNAT address) never hits the cap,
//! while a single abusive source is bounded. Tune with the constants below.

use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use actix_web::dev::ServiceRequest;
use parking_lot::Mutex;

/// Bucket capacity: the largest burst a single client may make back-to-back.
pub const BURST_CAPACITY: f64 = 200.0;
/// Sustained refill rate in tokens per second (5/s == 300/min sustained once the burst is spent).
pub const REFILL_PER_SEC: f64 = 5.0;
/// Drop a client's bucket after this many seconds of inactivity (keeps memory bounded).
const IDLE_EVICT_SECS: f64 = 900.0;
/// How often to sweep idle buckets.
const GC_INTERVAL_SECS: f64 = 300.0;

struct Bucket {
    tokens: f64,
    last: Instant,
}

struct Inner {
    buckets: HashMap<IpAddr, Bucket>,
    last_gc: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<Inner>>,
    capacity: f64,
    refill_per_sec: f64,
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_per_sec: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                buckets: HashMap::new(),
                last_gc: Instant::now(),
            })),
            capacity,
            refill_per_sec,
        }
    }

    /// Returns `true` if the request from `ip` is allowed (and consumes one token).
    pub fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut inner = self.inner.lock();

        // Opportunistic GC so the map can't grow without bound under churny client IPs.
        if now.duration_since(inner.last_gc).as_secs_f64() >= GC_INTERVAL_SECS {
            inner
                .buckets
                .retain(|_, b| now.duration_since(b.last).as_secs_f64() < IDLE_EVICT_SECS);
            inner.last_gc = now;
        }

        let capacity = self.capacity;
        let refill = self.refill_per_sec;
        let bucket = inner.buckets.entry(ip).or_insert(Bucket {
            tokens: capacity,
            last: now,
        });

        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * refill).min(capacity);
        bucket.last = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Extract the real client IP: `CF-Connecting-IP`, then the first `X-Forwarded-For` entry, then the
/// socket peer address. Falls back to a fixed sentinel so a header-less request is still counted.
pub fn client_ip(req: &ServiceRequest) -> IpAddr {
    if let Some(v) = req.headers().get("CF-Connecting-IP") {
        if let Ok(s) = v.to_str() {
            if let Ok(ip) = IpAddr::from_str(s.trim()) {
                return ip;
            }
        }
    }
    if let Some(v) = req.headers().get("X-Forwarded-For") {
        if let Ok(s) = v.to_str() {
            if let Some(first) = s.split(',').next() {
                if let Ok(ip) = IpAddr::from_str(first.trim()) {
                    return ip;
                }
            }
        }
    }
    req.peer_addr()
        .map(|a| a.ip())
        .unwrap_or(IpAddr::from([0u8, 0, 0, 0]))
}
