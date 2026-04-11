use crate::api::VersionResponse;
use crate::db::repository::CddRepository;
use crate::github::client::GitHubClient;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// A JSON-RPC 2.0 Request payload
#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct RpcRequest {
    /// JSON-RPC version (must be "2.0")
    #[schema(example = "2.0")]
    pub jsonrpc: String,
    /// The method to invoke
    #[schema(example = "version")]
    pub method: String,
    /// Parameters for the method (optional)
    #[schema(value_type = Option<Object>)]
    pub params: Option<serde_json::Value>,
    /// Request ID (optional)
    #[schema(value_type = Option<Object>)]
    pub id: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 Error object
#[derive(Serialize, Deserialize, Debug, PartialEq, ToSchema)]
pub struct RpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
}

/// A JSON-RPC 2.0 Response payload
#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct RpcResponse {
    /// JSON-RPC version (always "2.0")
    #[schema(example = "2.0")]
    pub jsonrpc: String,
    /// Successful result payload
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub result: Option<serde_json::Value>,
    /// Error payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// Correlated request ID
    #[schema(value_type = Option<Object>)]
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
#[utoipa::path(
    post,
    path = "/rpc",
    request_body = RpcRequest,
    responses(
        (status = 200, description = "JSON-RPC response", body = RpcResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn rpc_handler(
    req: web::Json<RpcRequest>,
    repo: web::Data<Arc<dyn CddRepository>>,
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
        "to_docs_json" => {
            let params = req.params.as_ref().and_then(|p| p.as_object());
            let target_language = params
                .and_then(|p| p.get("target_language"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let target = if target_language.starts_with("cdd-") {
                target_language.to_string()
            } else {
                format!("cdd-{}", target_language)
            };

            let is_wasm = std::env::var("WASM_EXECUTION_MODE").unwrap_or_default() == "1";
            if is_wasm
                && matches!(
                    target.as_str(),
                    "cdd-java"
                        | "cdd-python" | "cdd-python-all"
                        | "cdd-sh"
                        | "cdd-cpp"
                        | "cdd-csharp"
                        | "cdd-kotlin"
                        | "cdd-ruby"
                        | "cdd-ts"
                )
            {
                return HttpResponse::BadRequest().json(RpcResponse::error(
                    400,
                    format!("Error: The target '{}' is currently unsupported or unavailable for WebAssembly execution.", target),
                    req.id.clone(),
                ));
            }

            let input = params
                .and_then(|p| p.get("input"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let no_imports = params
                .and_then(|p| p.get("no_imports"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let no_wrapping = params
                .and_then(|p| p.get("no_wrapping"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if input.is_empty() {
                return HttpResponse::BadRequest().json(RpcResponse::error(
                    400,
                    "Missing 'input' parameter".to_string(),
                    req.id.clone(),
                ));
            }

            let mut cmd;
            if is_wasm {
                cmd = std::process::Command::new("wasmtime");
                if target == "cdd-kotlin" {
                    cmd.arg("--wasm-features=gc");
                }
                let input_path = std::path::Path::new(&input)
                    .canonicalize()
                    .unwrap_or_else(|_| std::path::PathBuf::from(&input));
                let input_dir = input_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(""));
                cmd.arg(format!("--dir={}::/workspace", input_dir.display()));
                let wasm_file = format!("cdd-ctl-wasm-sdk/assets/wasm/{}.wasm", target);
                cmd.arg(&wasm_file);
                cmd.arg("--");
                cmd.arg("to_docs_json");
                let filename = input_path.file_name().unwrap_or_default().to_string_lossy();
                cmd.arg("-i").arg(format!("/workspace/{}", filename));

                if no_imports {
                    cmd.arg("--no-imports");
                }
                if no_wrapping {
                    cmd.arg("--no-wrapping");
                }
            } else {
                cmd = std::process::Command::new(&target);
                cmd.arg("to_docs_json");
                cmd.arg("-i").arg(input);
                if no_imports {
                    cmd.arg("--no-imports");
                }
                if no_wrapping {
                    cmd.arg("--no-wrapping");
                }
            }

            match cmd.output() {
                Ok(output) => {
                    if output.status.success() {
                        let json_str = String::from_utf8_lossy(&output.stdout);
                        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                            HttpResponse::Ok().json(RpcResponse::success(json_val, req.id.clone()))
                        } else {
                            HttpResponse::InternalServerError().json(RpcResponse::error(
                                500,
                                "Invalid JSON generated by target".to_string(),
                                req.id.clone(),
                            ))
                        }
                    } else {
                        let err_str = String::from_utf8_lossy(&output.stderr);
                        HttpResponse::InternalServerError().json(RpcResponse::error(
                            output.status.code().unwrap_or(500),
                            format!("Target error: {}", err_str),
                            req.id.clone(),
                        ))
                    }
                }
                Err(e) => HttpResponse::InternalServerError().json(RpcResponse::error(
                    500,
                    format!("Failed to execute '{}': {}", target, e),
                    req.id.clone(),
                )),
            }
        }
        "get_organization" => {
            let params = req.params.as_ref().and_then(|p| p.as_object());
            let org_id = params
                .and_then(|p| p.get("org_id"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let Some(org_id) = org_id else {
                return HttpResponse::Ok().json(RpcResponse::error(
                    -32602,
                    "Missing required param: org_id".to_string(),
                    req.id.clone(),
                ));
            };
            match repo.get_organization(org_id).await {
                Ok(Some(org)) => HttpResponse::Ok().json(RpcResponse::success(
                    serde_json::to_value(org).unwrap(),
                    req.id.clone(),
                )),
                Ok(None) => HttpResponse::Ok().json(RpcResponse::error(
                    404,
                    format!("Organization {} not found", org_id),
                    req.id.clone(),
                )),
                Err(e) => HttpResponse::Ok().json(RpcResponse::error(
                    500,
                    format!("DB error: {}", e),
                    req.id.clone(),
                )),
            }
        }
        "get_repository" => {
            let params = req.params.as_ref().and_then(|p| p.as_object());
            let repo_id = params
                .and_then(|p| p.get("repo_id"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let Some(repo_id) = repo_id else {
                return HttpResponse::Ok().json(RpcResponse::error(
                    -32602,
                    "Missing required param: repo_id".to_string(),
                    req.id.clone(),
                ));
            };
            match repo.get_repository(repo_id).await {
                Ok(Some(r)) => HttpResponse::Ok().json(RpcResponse::success(
                    serde_json::to_value(r).unwrap(),
                    req.id.clone(),
                )),
                Ok(None) => HttpResponse::Ok().json(RpcResponse::error(
                    404,
                    format!("Repository {} not found", repo_id),
                    req.id.clone(),
                )),
                Err(e) => HttpResponse::Ok().json(RpcResponse::error(
                    500,
                    format!("DB error: {}", e),
                    req.id.clone(),
                )),
            }
        }
        "get_user_role" => {
            let params = req.params.as_ref().and_then(|p| p.as_object());
            let org_id = params
                .and_then(|p| p.get("org_id"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let user_id = params
                .and_then(|p| p.get("user_id"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let (Some(org_id), Some(user_id)) = (org_id, user_id) else {
                return HttpResponse::Ok().json(RpcResponse::error(
                    -32602,
                    "Missing required params: org_id, user_id".to_string(),
                    req.id.clone(),
                ));
            };
            match repo.get_user_role(org_id, user_id).await {
                Ok(Some(role)) => HttpResponse::Ok().json(RpcResponse::success(
                    serde_json::json!({ "role": role }),
                    req.id.clone(),
                )),
                Ok(None) => HttpResponse::Ok().json(RpcResponse::error(
                    404,
                    format!("User {} has no role in org {}", user_id, org_id),
                    req.id.clone(),
                )),
                Err(e) => HttpResponse::Ok().json(RpcResponse::error(
                    500,
                    format!("DB error: {}", e),
                    req.id.clone(),
                )),
            }
        }
        "create_organization" => {
            let params = req.params.as_ref().and_then(|p| p.as_object());
            let login = params
                .and_then(|p| p.get("login"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let Some(login) = login else {
                return HttpResponse::Ok().json(RpcResponse::error(
                    -32602,
                    "Missing required param: login".to_string(),
                    req.id.clone(),
                ));
            };
            let description = params
                .and_then(|p| p.get("description"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            match repo.create_organization(None, login, description).await {
                Ok(org) => HttpResponse::Ok().json(RpcResponse::success(
                    serde_json::to_value(org).unwrap(),
                    req.id.clone(),
                )),
                Err(e) => HttpResponse::Ok().json(RpcResponse::error(
                    500,
                    format!("DB error: {}", e),
                    req.id.clone(),
                )),
            }
        }
        "create_repository" => {
            let params = req.params.as_ref().and_then(|p| p.as_object());
            let org_id = params
                .and_then(|p| p.get("org_id"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let name = params
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let (Some(org_id), Some(name)) = (org_id, name) else {
                return HttpResponse::Ok().json(RpcResponse::error(
                    -32602,
                    "Missing required params: org_id, name".to_string(),
                    req.id.clone(),
                ));
            };
            let description = params
                .and_then(|p| p.get("description"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            match repo.create_repository(org_id, None, name, description).await {
                Ok(r) => HttpResponse::Ok().json(RpcResponse::success(
                    serde_json::to_value(r).unwrap(),
                    req.id.clone(),
                )),
                Err(e) => HttpResponse::Ok().json(RpcResponse::error(
                    500,
                    format!("DB error: {}", e),
                    req.id.clone(),
                )),
            }
        }
        _ => HttpResponse::Ok().json(RpcResponse::error(
            -32601,
            "Method not found".to_string(),
            req.id.clone(),
        )),
    }
}

/// Configure
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

    #[actix_web::test]
    async fn test_rpc_handler_to_docs_json_missing_input() {
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
                method: "to_docs_json".to_string(),
                params: Some(serde_json::json!({
                    "target_language": "cdd-c"
                })),
                id: Some(serde_json::json!(4)),
            })
            .to_request();

        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 400);
    }

    #[actix_web::test]
    async fn test_rpc_handler_to_docs_json_unsupported_wasm() {
        std::env::set_var("WASM_EXECUTION_MODE", "1");
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
                method: "to_docs_json".to_string(),
                params: Some(serde_json::json!({
                    "target_language": "java",
                    "input": "spec.json"
                })),
                id: Some(serde_json::json!(5)),
            })
            .to_request();

        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 400);
        std::env::remove_var("WASM_EXECUTION_MODE");
    }

    #[actix_web::test]
    async fn test_get_organization_happy_path() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_organization()
            .returning(|_| Ok(Some(crate::db::models::Organization {
                id: 1,
                github_id: None,
                login: "test-org".to_string(),
                description: None,
            })));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_organization".to_string(),
                params: Some(serde_json::json!({ "org_id": 1 })),
                id: Some(serde_json::json!(10)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[actix_web::test]
    async fn test_get_organization_missing_param() {
        let repo = MockCddRepository::new();
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_organization".to_string(),
                params: Some(serde_json::json!({})),
                id: Some(serde_json::json!(11)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[actix_web::test]
    async fn test_get_organization_not_found() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_organization().returning(|_| Ok(None));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_organization".to_string(),
                params: Some(serde_json::json!({ "org_id": 999 })),
                id: Some(serde_json::json!(12)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 404);
    }

    #[actix_web::test]
    async fn test_get_organization_db_error() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_organization()
            .returning(|_| Err(diesel::result::Error::NotFound));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_organization".to_string(),
                params: Some(serde_json::json!({ "org_id": 1 })),
                id: Some(serde_json::json!(13)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 500);
    }

    #[actix_web::test]
    async fn test_get_repository_happy_path() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_repository()
            .returning(|_| Ok(Some(crate::db::models::Repository {
                id: 1,
                organization_id: 1,
                github_id: None,
                name: "test-repo".to_string(),
                description: None,
            })));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_repository".to_string(),
                params: Some(serde_json::json!({ "repo_id": 1 })),
                id: Some(serde_json::json!(20)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[actix_web::test]
    async fn test_get_repository_missing_param() {
        let repo = MockCddRepository::new();
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_repository".to_string(),
                params: Some(serde_json::json!({})),
                id: Some(serde_json::json!(21)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[actix_web::test]
    async fn test_get_repository_not_found() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_repository().returning(|_| Ok(None));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_repository".to_string(),
                params: Some(serde_json::json!({ "repo_id": 999 })),
                id: Some(serde_json::json!(22)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 404);
    }

    #[actix_web::test]
    async fn test_get_repository_db_error() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_repository()
            .returning(|_| Err(diesel::result::Error::NotFound));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_repository".to_string(),
                params: Some(serde_json::json!({ "repo_id": 1 })),
                id: Some(serde_json::json!(23)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 500);
    }

    #[actix_web::test]
    async fn test_get_user_role_happy_path() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_user_role()
            .returning(|_, _| Ok(Some("owner".to_string())));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_user_role".to_string(),
                params: Some(serde_json::json!({ "org_id": 1, "user_id": 2 })),
                id: Some(serde_json::json!(30)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.result.is_some());
        assert_eq!(resp.result.unwrap()["role"], "owner");
    }

    #[actix_web::test]
    async fn test_get_user_role_missing_param() {
        let repo = MockCddRepository::new();
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_user_role".to_string(),
                params: Some(serde_json::json!({ "org_id": 1 })),
                id: Some(serde_json::json!(31)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[actix_web::test]
    async fn test_get_user_role_not_found() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_user_role().returning(|_, _| Ok(None));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_user_role".to_string(),
                params: Some(serde_json::json!({ "org_id": 1, "user_id": 999 })),
                id: Some(serde_json::json!(32)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 404);
    }

    #[actix_web::test]
    async fn test_get_user_role_db_error() {
        let mut repo = MockCddRepository::new();
        repo.expect_get_user_role()
            .returning(|_, _| Err(diesel::result::Error::NotFound));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "get_user_role".to_string(),
                params: Some(serde_json::json!({ "org_id": 1, "user_id": 2 })),
                id: Some(serde_json::json!(33)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 500);
    }

    #[actix_web::test]
    async fn test_create_organization_happy_path() {
        let mut repo = MockCddRepository::new();
        repo.expect_create_organization()
            .returning(|_, _, _| Ok(crate::db::models::Organization {
                id: 1,
                github_id: None,
                login: "new-org".to_string(),
                description: None,
            }));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "create_organization".to_string(),
                params: Some(serde_json::json!({ "login": "new-org" })),
                id: Some(serde_json::json!(40)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[actix_web::test]
    async fn test_create_organization_missing_param() {
        let repo = MockCddRepository::new();
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "create_organization".to_string(),
                params: Some(serde_json::json!({})),
                id: Some(serde_json::json!(41)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[actix_web::test]
    async fn test_create_organization_db_error() {
        let mut repo = MockCddRepository::new();
        repo.expect_create_organization()
            .returning(|_, _, _| Err(diesel::result::Error::NotFound));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "create_organization".to_string(),
                params: Some(serde_json::json!({ "login": "new-org" })),
                id: Some(serde_json::json!(42)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 500);
    }

    #[actix_web::test]
    async fn test_create_repository_happy_path() {
        let mut repo = MockCddRepository::new();
        repo.expect_create_repository()
            .returning(|_, _, _, _| Ok(crate::db::models::Repository {
                id: 1,
                organization_id: 1,
                github_id: None,
                name: "new-repo".to_string(),
                description: None,
            }));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "create_repository".to_string(),
                params: Some(serde_json::json!({ "org_id": 1, "name": "new-repo" })),
                id: Some(serde_json::json!(50)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[actix_web::test]
    async fn test_create_repository_missing_param() {
        let repo = MockCddRepository::new();
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "create_repository".to_string(),
                params: Some(serde_json::json!({ "org_id": 1 })),
                id: Some(serde_json::json!(51)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[actix_web::test]
    async fn test_create_repository_db_error() {
        let mut repo = MockCddRepository::new();
        repo.expect_create_repository()
            .returning(|_, _, _, _| Err(diesel::result::Error::NotFound));
        let gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        ).await;
        let req = test::TestRequest::post().uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "create_repository".to_string(),
                params: Some(serde_json::json!({ "org_id": 1, "name": "new-repo" })),
                id: Some(serde_json::json!(52)),
            }).to_request();
        let resp: RpcResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.error.unwrap().code, 500);
    }
}
