#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Reverse proxy handler for forwarding unmatched routes to upstream services.

use crate::config::AppConfig;
use crate::error::CddGatewayError;
use actix_web::{web, HttpRequest, HttpResponse};
use reqwest::Client;

/// Returns `true` if `header_name` is an RFC 9110 hop-by-hop header that must not be
/// forwarded across a reverse proxy boundary.
///
/// # Arguments
/// * `header_name` - Lowercase or mixed-case HTTP header name.
///
/// # Returns
/// * `bool` - `true` if the header is hop-by-hop or `host`, `false` otherwise.
#[must_use]
pub fn is_hop_by_hop_header(header_name: &str) -> bool {
    matches!(
        header_name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
            | "host"
    )
}

/// Forwards an incoming [`HttpRequest`] to the appropriate upstream backend.
///
/// Routes requests to `control_plane_url`, `docs_ui_url`, or `web_ui_url` configured in [`AppConfig`].
///
/// # Arguments
/// * `req` - Incoming HTTP request containing method, URI, and headers.
/// * `body` - Raw request payload bytes.
/// * `config` - Shared [`AppConfig`] containing upstream target URLs.
/// * `client` - Shared [`reqwest::Client`] used to issue the upstream request.
///
/// # Returns
/// * `Ok(HttpResponse)` - Proxied upstream HTTP status, headers, and body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If the request HTTP method cannot be parsed by `reqwest`.
/// * [`CddGatewayError::BadGateway`] - If communicating with or reading from the upstream server fails.
#[allow(clippy::future_not_send)]
pub async fn proxy_handler(
    req: HttpRequest,
    body: web::Bytes,
    config: web::Data<AppConfig>,
    client: web::Data<Client>,
) -> Result<HttpResponse, CddGatewayError> {
    let path = req.uri().path();
    let query = req
        .uri()
        .query()
        .map_or_else(String::new, |q| format!("?{q}"));

    let base_url = if path.starts_with("/api/") || path.starts_with("/auth/") {
        &config.control_plane_url
    } else if path.starts_with("/docs/") || path == "/docs" {
        &config.docs_ui_url
    } else {
        &config.web_ui_url
    };

    let normalized_base = base_url.trim_end_matches('/');
    let url = format!("{normalized_base}{path}{query}");

    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);

    let mut proxy_req = client.request(method, &url);

    for (header_name, header_value) in req.headers() {
        if !is_hop_by_hop_header(header_name.as_str()) {
            proxy_req = proxy_req.header(header_name.as_str(), header_value.as_bytes());
        }
    }

    proxy_req = proxy_req.body(body.to_vec());

    let res = proxy_req
        .send()
        .await
        .map_err(|e| CddGatewayError::BadGateway(format!("Proxy error: {e}")))?;

    let status_code = actix_web::http::StatusCode::from_u16(res.status().as_u16())
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);

    let mut response_builder = HttpResponse::build(status_code);

    for (header_name, header_value) in res.headers() {
        if !is_hop_by_hop_header(header_name.as_str()) {
            response_builder.insert_header((header_name.as_str(), header_value.as_bytes()));
        }
    }

    let res_body = res.bytes().await?;

    Ok(response_builder.body(res_body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use httpmock::prelude::*;

    #[actix_web::test]
    async fn test_is_hop_by_hop_header() {
        assert!(is_hop_by_hop_header("Connection"));
        assert!(is_hop_by_hop_header("Transfer-Encoding"));
        assert!(is_hop_by_hop_header("Host"));
        assert!(!is_hop_by_hop_header("X-Custom-Header"));
    }

    #[actix_web::test]
    async fn test_proxy_routing() {
        let control_plane = MockServer::start();
        let docs_ui = MockServer::start();
        let web_ui = MockServer::start();

        let cp_mock = control_plane.mock(|when, then| {
            when.method(GET)
                .path("/api/users")
                .query_param("page", "1")
                .header("X-Custom", "val");
            then.status(200)
                .header("X-Resp", "resp_val")
                .header("Connection", "keep-alive")
                .body("cp_body");
        });

        let docs_mock = docs_ui.mock(|when, then| {
            when.method(GET).path("/docs/intro");
            then.status(200).body("docs_body");
        });

        let web_mock = web_ui.mock(|when, then| {
            when.method(GET).path("/index.html");
            then.status(200).body("web_body");
        });

        let config = AppConfig {
            control_plane_url: format!("{}/", control_plane.base_url()),
            docs_ui_url: docs_ui.base_url(),
            web_ui_url: web_ui.base_url(),
            ..Default::default()
        };

        let client = Client::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(client))
                .default_service(web::route().to(proxy_handler)),
        )
        .await;

        // 1. Test Control Plane Routing (/api/users?page=1) + Hop-by-hop stripping
        let req1 = test::TestRequest::get()
            .uri("/api/users?page=1")
            .insert_header(("X-Custom", "val"))
            .insert_header(("Connection", "keep-alive"))
            .to_request();
        let resp1 = test::call_service(&app, req1).await;
        assert_eq!(resp1.status(), actix_web::http::StatusCode::OK);
        assert_eq!(
            resp1.headers().get("X-Resp").and_then(|v| v.to_str().ok()),
            Some("resp_val")
        );
        assert!(resp1.headers().get("Connection").is_none());
        let body1 = test::read_body(resp1).await;
        assert_eq!(body1, "cp_body");
        cp_mock.assert();

        // 2. Test Docs UI Routing (/docs/intro)
        let req2 = test::TestRequest::get().uri("/docs/intro").to_request();
        let resp2 = test::call_service(&app, req2).await;
        assert_eq!(resp2.status(), actix_web::http::StatusCode::OK);
        let body2 = test::read_body(resp2).await;
        assert_eq!(body2, "docs_body");
        docs_mock.assert();

        // 3. Test Web UI Routing (/index.html)
        let req3 = test::TestRequest::get().uri("/index.html").to_request();
        let resp3 = test::call_service(&app, req3).await;
        assert_eq!(resp3.status(), actix_web::http::StatusCode::OK);
        let body3 = test::read_body(resp3).await;
        assert_eq!(body3, "web_body");
        web_mock.assert();
    }

    #[actix_web::test]
    async fn test_proxy_bad_gateway_and_docs_root() {
        let docs_ui = MockServer::start();
        let docs_mock = docs_ui.mock(|when, then| {
            when.method(GET).path("/docs");
            then.status(200)
                .header("X-Custom", "custom_val")
                .body("docs_root");
        });

        let config = AppConfig {
            control_plane_url: "http://127.0.0.1:1".to_string(),
            docs_ui_url: docs_ui.base_url(),
            web_ui_url: "http://127.0.0.1:1".to_string(),
            ..Default::default()
        };

        let client = Client::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(client))
                .default_service(web::route().to(proxy_handler)),
        )
        .await;

        let req_bad = test::TestRequest::get().uri("/auth/login").to_request();
        let resp_bad = test::call_service(&app, req_bad).await;
        assert_eq!(resp_bad.status(), actix_web::http::StatusCode::BAD_GATEWAY);

        let req_docs = test::TestRequest::get()
            .uri("/docs")
            .insert_header(("X-Custom", "custom_val"))
            .to_request();
        let resp_docs = test::call_service(&app, req_docs).await;
        assert_eq!(resp_docs.status(), actix_web::http::StatusCode::OK);
        docs_mock.assert();
    }
}
