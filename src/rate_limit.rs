//! Rate limiting middleware

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
    Error, HttpResponse,
};
use std::{
    collections::HashMap,
    future::{ready, Future, Ready},
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Simple in-memory rate limiter state
#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    /// Create a new RateLimiter
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    /// Check if a request is allowed
    pub fn check(&self, ip: &str) -> bool {
        let mut reqs = self
            .requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Instant::now();
        let ip_reqs = reqs.entry(ip.to_string()).or_default();

        // Remove old requests
        ip_reqs.retain(|&t| now.duration_since(t) < self.window);

        if ip_reqs.len() < self.max_requests {
            ip_reqs.push(now);
            true
        } else {
            false
        }
    }
}

/// Rate limit middleware factory
pub struct RateLimit {
    limiter: RateLimiter,
}

impl RateLimit {
    /// Create new RateLimit middleware
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            limiter: RateLimiter::new(max_requests, window),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimitMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddleware {
            service,
            limiter: self.limiter.clone(),
        }))
    }
}

/// Rate limit middleware implementation
pub struct RateLimitMiddleware<S> {
    service: S,
    limiter: RateLimiter,
}

type EitherBody<B> = actix_web::body::EitherBody<B>;

impl<S, B> Service<ServiceRequest> for RateLimitMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let ip = req
            .peer_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if !self.limiter.check(&ip) {
            let (request, _pl) = req.into_parts();
            let response =
                HttpResponse::build(StatusCode::TOO_MANY_REQUESTS).body("Too Many Requests");
            return Box::pin(ready(Ok(ServiceResponse::new(
                request,
                response.map_into_right_body(),
            ))));
        }

        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res.map_into_left_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};

    async fn index() -> HttpResponse {
        HttpResponse::Ok().finish()
    }

    #[actix_web::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.1"));
        assert!(!limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.2"));
    }

    #[actix_web::test]
    async fn test_rate_limit_middleware() {
        let app = test::init_service(
            App::new()
                .wrap(RateLimit::new(1, Duration::from_secs(60)))
                .route("/", web::get().to(index)),
        )
        .await;

        let req1 = test::TestRequest::get().uri("/").to_request();
        let resp1 = test::call_service(&app, req1).await;
        assert_eq!(resp1.status(), StatusCode::OK);

        // Note: test::TestRequest doesn't set peer_addr by default,
        // so it defaults to "unknown". Thus both share "unknown" IP.
        let req2 = test::TestRequest::get().uri("/").to_request();
        let resp2 = test::call_service(&app, req2).await;
        assert_eq!(resp2.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
