//! Proxy module for routing requests to the appropriate backend

use crate::config::AppConfig;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use reqwest::Client;

/// Proxy handler to forward requests to the appropriate backend
pub async fn proxy_handler(
    req: HttpRequest,
    bytes: web::Bytes,
    client: web::Data<Client>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse, Error> {
    let path = req.uri().path();
    let query = req.uri().query().unwrap_or("");

    let base_url = if path.starts_with("/api/") || path.starts_with("/auth/") {
        &config.control_plane_url
    } else if path.starts_with("/u/") {
        &config.docs_ui_url
    } else {
        &config.web_ui_url
    };

    let mut url = format!("{}{}", base_url, path);
    if !query.is_empty() {
        url.push('?');
        url.push_str(query);
    }

    let method_str = req.method().as_str();
    let reqwest_method =
        reqwest::Method::from_bytes(method_str.as_bytes()).unwrap_or(reqwest::Method::GET);

    let mut proxy_req = client.request(reqwest_method, url);

    for (key, value) in req.headers() {
        if key != actix_web::http::header::HOST {
            proxy_req = proxy_req.header(key.as_str(), value.as_bytes());
        }
    }

    let proxy_resp = match proxy_req.body(bytes).send().await {
        Ok(res) => res,
        Err(e) => {
            log::error!("Proxy request failed: {}", e);
            return Ok(HttpResponse::BadGateway().body("Bad Gateway"));
        }
    };

    let status = actix_web::http::StatusCode::from_u16(proxy_resp.status().as_u16())
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);

    let mut builder = HttpResponse::build(status);
    for (key, value) in proxy_resp.headers() {
        if let Ok(name) = actix_web::http::header::HeaderName::from_bytes(key.as_str().as_bytes()) {
            if let Ok(val) = actix_web::http::header::HeaderValue::from_bytes(value.as_bytes()) {
                builder.insert_header((name, val));
            }
        }
    }

    Ok(builder.streaming(proxy_resp.bytes_stream()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use reqwest::Client;

    #[actix_web::test]
    async fn test_proxy_handler() {
        let config = AppConfig {
            database_url: "".into(),
            server_bind: "".into(),
            jwt_secret: "".into(),
            webhook_secret: "".into(),
            github_token: None,
            offline_mode: true,
            control_plane_url: "http://127.0.0.1:0".into(),
            docs_ui_url: "http://127.0.0.1:0".into(),
            web_ui_url: "http://127.0.0.1:0".into(),
            servers: Default::default(),
        };
        let client = Client::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(client))
                .app_data(web::Data::new(config))
                .default_service(web::route().to(proxy_handler)),
        )
        .await;

        let req = test::TestRequest::get().uri("/api/test").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_GATEWAY);

        let req_auth = test::TestRequest::get().uri("/auth/login").to_request();
        let resp_auth = test::call_service(&app, req_auth).await;
        assert_eq!(resp_auth.status(), actix_web::http::StatusCode::BAD_GATEWAY);

        let req = test::TestRequest::get().uri("/u/test?q=1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_GATEWAY);

        let req = test::TestRequest::get().uri("/test").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_GATEWAY);
    }

    #[actix_web::test]
    async fn test_proxy_handler_success() {
        use httpmock::prelude::*;
        use httpmock::MockServer;

        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(GET).path("/api/test_success");
            then.status(200)
                .header("X-Test-Header", "test_value")
                .body("Success!");
        });

        let config = AppConfig {
            database_url: "".into(),
            server_bind: "".into(),
            jwt_secret: "".into(),
            webhook_secret: "".into(),
            github_token: None,
            offline_mode: true,
            control_plane_url: server.base_url(),
            docs_ui_url: server.base_url(),
            web_ui_url: server.base_url(),
            servers: Default::default(),
        };
        let client = Client::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(client))
                .app_data(web::Data::new(config))
                .default_service(web::route().to(proxy_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/test_success")
            .insert_header(("Host", "localhost"))
            .insert_header(("X-Custom", "custom_val"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

        mock.assert();
    }
}
