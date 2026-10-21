#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Sliding-window per-IP rate limiting middleware for Actix-web.

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
    Error, HttpResponse,
};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Thread-safe sliding-window rate limiter keyed by client IP address.
///
/// Automatically evicts expired timestamp vectors from the internal map so stale
/// client IP entries never leak memory.
#[derive(Clone)]
pub struct RateLimiter {
    /// Map of client IP string to recent request timestamps within the current window.
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    /// Maximum number of requests allowed per client IP within `window`.
    max_requests: usize,
    /// Sliding window duration.
    window: Duration,
}

impl RateLimiter {
    /// Creates a new [`RateLimiter`] enforcing `max_requests` per `window`.
    ///
    /// # Arguments
    /// * `max_requests` - Maximum number of requests permitted per IP within `window`.
    /// * `window` - Sliding window duration.
    ///
    /// # Returns
    /// * [`RateLimiter`] - Initialized rate limiter state.
    #[must_use]
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    /// Checks whether a request from `ip` is permitted and records the current timestamp if allowed.
    ///
    /// Prunes expired timestamps across tracked keys and removes keys whose timestamp vectors
    /// become empty.
    ///
    /// # Arguments
    /// * `&self` - The rate limiter state.
    /// * `ip` - Client IP address string slice.
    ///
    /// # Returns
    /// * `bool` - `true` if the request is within limits, `false` if rate-limited.
    #[must_use]
    pub fn check(&self, ip: &str) -> bool {
        let now = Instant::now();
        let window = self.window;
        let max_requests = self.max_requests;

        let mut reqs = match self.requests.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        reqs.retain(|_, timestamps| {
            timestamps.retain(|&t| now.duration_since(t) < window);
            !timestamps.is_empty()
        });

        let entry = reqs.entry(ip.to_string()).or_default();
        let allowed = entry.len() < max_requests;
        if allowed {
            entry.push(now);
        }
        drop(reqs);
        allowed
    }

    /// Returns the number of currently tracked active IP keys in the internal map.
    ///
    /// # Arguments
    /// * `&self` - The rate limiter state.
    ///
    /// # Returns
    /// * `usize` - Number of active IP keys with non-expired timestamps.
    #[must_use]
    pub fn active_ip_count(&self) -> usize {
        let reqs = match self.requests.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        reqs.len()
    }
}

/// Actix-web middleware transform that applies [`RateLimiter`] to incoming requests.
pub struct RateLimit {
    /// Underlying shared rate limiter state.
    limiter: RateLimiter,
}

impl RateLimit {
    /// Creates a new [`RateLimit`] middleware factory.
    ///
    /// # Arguments
    /// * `max_requests` - Maximum number of requests permitted per IP within `window`.
    /// * `window` - Sliding window duration.
    ///
    /// # Returns
    /// * [`RateLimit`] - Configured middleware transform.
    #[must_use]
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            limiter: RateLimiter::new(max_requests, window),
        }
    }
}

impl<S> Transform<S, ServiceRequest> for RateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error> + 'static,
    S::Future: 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimitMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    /// Wraps the inner service `service` with [`RateLimitMiddleware`].
    ///
    /// # Arguments
    /// * `&self` - The middleware transform factory.
    /// * `service` - Inner Actix-web service.
    ///
    /// # Returns
    /// * `Self::Future` - Ready future yielding [`RateLimitMiddleware`].
    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddleware {
            service,
            limiter: self.limiter.clone(),
        }))
    }
}

/// Actix-web middleware service enforcing per-IP sliding-window rate limits.
pub struct RateLimitMiddleware<S> {
    /// Wrapped downstream Actix-web service.
    service: S,
    /// Shared rate limiter instance.
    limiter: RateLimiter,
}

impl<S> Service<ServiceRequest> for RateLimitMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error> + 'static,
    S::Future: 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    /// Evaluates the client's rate limit and either forwards the request or returns `429 Too Many Requests`.
    ///
    /// # Arguments
    /// * `&self` - The middleware service instance.
    /// * `req` - Incoming [`ServiceRequest`].
    ///
    /// # Returns
    /// * `Self::Future` - Future resolving to the downstream response or a `429` response.
    fn call(&self, req: ServiceRequest) -> Self::Future {
        let ip = req
            .peer_addr()
            .map_or_else(|| "unknown".to_string(), |addr| addr.ip().to_string());

        if self.limiter.check(&ip) {
            let fut = self.service.call(req);
            Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            })
        } else {
            Box::pin(async move {
                let res = req.into_response(
                    HttpResponse::build(StatusCode::TOO_MANY_REQUESTS)
                        .body("Rate limit exceeded")
                        .map_into_boxed_body(),
                );
                Ok(res)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    async fn test_handler() -> HttpResponse {
        HttpResponse::Ok().body("ok")
    }

    #[actix_web::test]
    async fn test_rate_limiter_evicts_expired_ips() {
        let limiter = RateLimiter::new(2, Duration::from_millis(20));
        assert!(limiter.check("10.0.0.1"));
        assert_eq!(limiter.active_ip_count(), 1);

        std::thread::sleep(Duration::from_millis(30));

        // Checking another IP prunes expired entries from the map
        assert!(limiter.check("10.0.0.2"));
        assert_eq!(limiter.active_ip_count(), 1);
    }

    #[actix_web::test]
    async fn test_rate_limiter_poisoned_mutex_recovery() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        let limiter_clone = limiter.clone();
        let _ = std::thread::spawn(move || {
            let _guard = limiter_clone.requests.lock();
            panic!("poison");
        })
        .join();

        assert!(limiter.check("127.0.0.1"));
        assert_eq!(limiter.active_ip_count(), 1);
    }

    #[actix_web::test]
    async fn test_rate_limit_middleware() {
        let app = test::init_service(
            App::new()
                .wrap(RateLimit::new(2, Duration::from_secs(10)))
                .route("/test", web::get().to(test_handler)),
        )
        .await;

        let req1 = test::TestRequest::get().uri("/test").to_request();
        let resp1 = test::call_service(&app, req1).await;
        assert_eq!(resp1.status(), StatusCode::OK);

        let req2 = test::TestRequest::get().uri("/test").to_request();
        let resp2 = test::call_service(&app, req2).await;
        assert_eq!(resp2.status(), StatusCode::OK);

        let req3 = test::TestRequest::get().uri("/test").to_request();
        let resp3 = test::call_service(&app, req3).await;
        assert_eq!(resp3.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[actix_web::test]
    async fn test_rate_limit_with_peer_addr() {
        let app = test::init_service(
            App::new()
                .wrap(RateLimit::new(1, Duration::from_secs(10)))
                .route("/test", web::get().to(test_handler)),
        )
        .await;

        let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let req = test::TestRequest::get()
            .uri("/test")
            .peer_addr(peer)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
