use crate::api::VersionResponse;
use crate::db::repository::CddRepository;
use crate::github::client::GitHubClient;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A JSON-RPC 2.0 Request payload
#[derive(Serialize, Deserialize, Debug)]
pub struct RpcRequest {
    /// JSON-RPC version (must be "2.0")
    pub jsonrpc: String,
    /// The method to invoke
    pub method: String,
    /// Parameters for the method (optional)
    pub params: Option<serde_json::Value>,
    /// Request ID (optional)
    pub id: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 Error object
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
}

/// A JSON-RPC 2.0 Response payload
#[derive(Serialize, Deserialize, Debug)]
pub struct RpcResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Successful result payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// Correlated request ID
    pub id: Option<serde_json::Value>,
}

impl RpcResponse {
    /// Create a success response
    pub fn success(result: serde_json::Value, id: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(code: i32, message: String, id: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError { code, message }),
            id,
        }
    }
}

/// The core JSON-RPC request handler
pub async fn rpc_handler(
    req: web::Json<RpcRequest>,
    _repo: web::Data<Arc<dyn CddRepository>>,
    _github_client: web::Data<Arc<dyn GitHubClient>>,
) -> impl Responder {
    if req.jsonrpc != "2.0" {
        return HttpResponse::Ok().json(RpcResponse::error(
            -32600,
            "Invalid Request".to_string(),
            req.id.clone(),
        ));
    }

    match req.method.as_str() {
        "version" => {
            let res = VersionResponse {
                version: env!("CARGO_PKG_VERSION").to_string(),
            };
            HttpResponse::Ok().json(RpcResponse::success(
                serde_json::to_value(res).unwrap(),
                req.id.clone(),
            ))
        }
        _ => HttpResponse::Ok().json(RpcResponse::error(
            -32601,
            "Method not found".to_string(),
            req.id.clone(),
        )),
    }
}

/// Configure the JSON-RPC route
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/rpc", web::post().to(rpc_handler));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::MockCddRepository;
    use crate::github::client::MockGitHubClient;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_rpc_handler_version() {
        let repo = MockCddRepository::new();
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "version".to_string(),
                params: None,
                id: Some(serde_json::json!(1)),
            })
            .to_request();

        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[actix_web::test]
    async fn test_rpc_handler_invalid_version() {
        let repo = MockCddRepository::new();
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "1.0".to_string(),
                method: "version".to_string(),
                params: None,
                id: Some(serde_json::json!(2)),
            })
            .to_request();

        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, -32600);
    }

    #[actix_web::test]
    async fn test_rpc_handler_method_not_found() {
        let repo = MockCddRepository::new();
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "unknown_method".to_string(),
                params: None,
                id: Some(serde_json::json!(3)),
            })
            .to_request();

        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, -32601);
    }
}
