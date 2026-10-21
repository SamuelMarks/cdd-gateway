#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! JSON-RPC 2.0 endpoint (`POST /rpc`) for control-plane queries and `to_docs_json` execution.

use crate::api::VersionResponse;
use crate::config::{AppConfig, ExecutionMode};
use crate::db::repository::CddRepository;
use crate::error::CddGatewayError;
use actix_web::{web, HttpResponse, Responder};
use async_trait::async_trait;
use cdd_engine::wasm_executor::{WasmExecutor, WASM_EXECUTOR};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

/// JSON-RPC 2.0 request envelope.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct RpcRequest {
    /// JSON-RPC protocol version string (must be `"2.0"`).
    pub jsonrpc: String,
    /// Name of the RPC method to invoke.
    pub method: String,
    /// Optional structured method parameters.
    pub params: Option<Value>,
    /// Optional request identifier echoed in the response.
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 response envelope.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct RpcResponse {
    /// JSON-RPC protocol version string (`"2.0"`).
    pub jsonrpc: String,
    /// Successful method result payload, if the call succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error object, if the call failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// Echoed request identifier.
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 error object.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct RpcError {
    /// Numeric JSON-RPC error code.
    pub code: i32,
    /// Human-readable error description.
    pub message: String,
    /// Optional additional error context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcResponse {
    /// Constructs a successful [`RpcResponse`] with the given `id` and `result`.
    ///
    /// # Arguments
    /// * `id` - Request identifier echoed back to the caller.
    /// * `result` - JSON value returned by the method.
    ///
    /// # Returns
    /// * [`RpcResponse`] - Success response envelope.
    #[must_use]
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Constructs a failed [`RpcResponse`] with the given `id`, `code`, and `message`.
    ///
    /// # Arguments
    /// * `id` - Request identifier echoed back to the caller.
    /// * `code` - Numeric JSON-RPC error code.
    /// * `message` - Human-readable error message.
    ///
    /// # Returns
    /// * [`RpcResponse`] - Error response envelope.
    #[must_use]
    pub fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

/// Typed parameters for the `to_docs_json` JSON-RPC method.
#[derive(Deserialize)]
pub struct ToDocsJsonParams {
    /// Target language identifier (e.g., `"rust"`, `"python-all"`, `"cdd-go"`).
    pub target_language: String,
    /// Path to the `OpenAPI` specification file to compile into documentation JSON.
    pub input: String,
    /// Whether to omit imports in generated code snippets.
    #[serde(default)]
    pub no_imports: bool,
    /// Whether to omit wrapping boilerplate in generated code snippets.
    #[serde(default)]
    pub no_wrapping: bool,
}

/// Typed parameters for the `get_organization` JSON-RPC method.
#[derive(Deserialize)]
struct GetOrgParams {
    /// Numeric organization ID to look up.
    org_id: i32,
}

/// Typed parameters for the `get_repository` JSON-RPC method.
#[derive(Deserialize)]
struct GetRepoParams {
    /// Numeric repository ID to look up.
    repo_id: i32,
}

/// Typed parameters for the `get_user_role` JSON-RPC method.
#[derive(Deserialize)]
struct GetUserRoleParams {
    /// Numeric organization ID.
    org_id: i32,
    /// Numeric user ID.
    user_id: i32,
}

/// Typed parameters for the `create_organization` JSON-RPC method.
#[derive(Deserialize)]
struct CreateOrgParams {
    /// Unique organization handle.
    login: String,
    /// Optional organization description.
    description: Option<String>,
    /// Optional GitHub numeric organization ID.
    github_id: Option<i64>,
}

/// Typed parameters for the `create_repository` JSON-RPC method.
#[derive(Deserialize)]
struct CreateRepoParams {
    /// Owning organization ID.
    org_id: i32,
    /// Repository name.
    name: String,
    /// Optional repository description.
    description: Option<String>,
    /// Optional GitHub numeric repository ID.
    github_id: Option<i64>,
}

/// Canonical list of allowed CDD target binary names.
const ALLOWED_TARGETS: &[&str] = &[
    "cdd-c",
    "cdd-cpp",
    "cdd-csharp",
    "cdd-go",
    "cdd-java",
    "cdd-kotlin",
    "cdd-php",
    "cdd-python-all",
    "cdd-ruby",
    "cdd-rust",
    "cdd-sh",
    "cdd-swift",
    "cdd-ts",
];

/// Resolves and validates a `target_language` parameter into a canonical `cdd-<lang>` binary name.
///
/// Rejects path traversal sequences (`..`, `/`) and unknown target names to prevent
/// arbitrary command execution.
///
/// # Arguments
/// * `raw` - Raw `target_language` string from the JSON-RPC request.
///
/// # Returns
/// * `Ok(String)` - Validated canonical target name (e.g., `"cdd-rust"`).
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `raw` is empty, contains path separators, or is not in the allowlist.
pub fn resolve_target_binary(raw: &str) -> Result<String, CddGatewayError> {
    if raw.is_empty() || raw.contains('/') || raw.contains("..") {
        return Err(CddGatewayError::Validation(format!(
            "Invalid target_language: {raw}"
        )));
    }
    let normalized = if raw.starts_with("cdd-") {
        raw.to_string()
    } else {
        format!("cdd-{raw}")
    };
    if ALLOWED_TARGETS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(CddGatewayError::Validation(format!(
            "Unsupported target_language: {raw}"
        )))
    }
}

/// Trait abstracting execution of `to_docs_json` across native subprocesses and WASM sandboxes.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DocsJsonExecutor: Send + Sync {
    /// Executes `to_docs_json` for a validated `target` binary and parameters.
    ///
    /// # Arguments
    /// * `target` - Canonical target binary name (e.g. `"cdd-rust"`).
    /// * `params` - Validated `to_docs_json` execution parameters.
    /// * `mode` - Execution mode (`Native` or `Wasm`).
    ///
    /// # Returns
    /// * `Ok(Value)` - Parsed JSON documentation output from the compiler.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If execution fails or output is not valid JSON.
    async fn execute(
        &self,
        target: &str,
        params: &ToDocsJsonParams,
        mode: ExecutionMode,
    ) -> Result<Value, CddGatewayError>;
}

/// Default [`DocsJsonExecutor`] implementation supporting both WASM and native CLI execution.
#[derive(Default)]
pub struct DefaultDocsJsonExecutor {
    /// Optional directory prefix where native `cdd-*` binaries reside (used in tests).
    pub bin_dir: Option<std::path::PathBuf>,
}

#[async_trait]
impl DocsJsonExecutor for DefaultDocsJsonExecutor {
    async fn execute(
        &self,
        target: &str,
        params: &ToDocsJsonParams,
        mode: ExecutionMode,
    ) -> Result<Value, CddGatewayError> {
        let wasm_supported = matches!(mode, ExecutionMode::Wasm)
            && !matches!(target, "cdd-java" | "cdd-kotlin" | "cdd-ruby" | "cdd-swift");

        let stdout = if wasm_supported {
            let mut args = vec!["to_docs_json".to_string()];
            if params.no_imports {
                args.push("--no-imports".to_string());
            }
            if params.no_wrapping {
                args.push("--no-wrapping".to_string());
            }
            WASM_EXECUTOR
                .execute_to_stdout(target, &params.input, &args)
                .map_err(|e| CddGatewayError::Internal(format!("WASM execution failed: {e}")))?
        } else {
            let program = self
                .bin_dir
                .as_ref()
                .map_or_else(|| std::path::PathBuf::from(target), |dir| dir.join(target));
            let mut cmd = std::process::Command::new(&program);
            cmd.arg("to_docs_json").arg("-i").arg(&params.input);
            if params.no_imports {
                cmd.arg("--no-imports");
            }
            if params.no_wrapping {
                cmd.arg("--no-wrapping");
            }

            let output = cmd.output().map_err(|e| {
                CddGatewayError::Internal(format!("Failed to execute {target}: {e}"))
            })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CddGatewayError::Internal(format!(
                    "Command {target} failed: {stderr}"
                )));
            }

            output.stdout
        };

        let stdout_str = String::from_utf8_lossy(&stdout);
        serde_json::from_str(&stdout_str).map_err(|e| {
            CddGatewayError::Internal(format!("Failed to parse JSON output from {target}: {e}"))
        })
    }
}

/// Converts a `serde_json::to_value` result into a [`RpcResponse`] without panicking.
///
/// # Arguments
/// * `id` - Echoed JSON-RPC request ID.
/// * `res` - Result of serializing the response payload into a JSON [`Value`].
///
/// # Returns
/// * [`HttpResponse`] - `200 OK` JSON response containing the serialized result or error.
fn ok_rpc_json(id: Option<Value>, res: Result<Value, serde_json::Error>) -> HttpResponse {
    match res {
        Ok(v) => HttpResponse::Ok().json(RpcResponse::success(id, v)),
        Err(e) => HttpResponse::InternalServerError().json(RpcResponse::error(
            id,
            -32603,
            format!("Serialization error: {e}"),
        )),
    }
}

/// Handles incoming JSON-RPC 2.0 requests (`POST /rpc`).
///
/// # Arguments
/// * `req` - Parsed [`RpcRequest`] JSON payload.
/// * `repo` - Database repository handle.
/// * `cfg` - Optional application configuration.
/// * `executor` - Optional [`DocsJsonExecutor`] override.
///
/// # Returns
/// * `impl Responder` - JSON-RPC 2.0 [`HttpResponse`].
#[utoipa::path(
    post,
    path = "/rpc",
    request_body = RpcRequest,
    responses(
        (status = 200, description = "JSON-RPC Response", body = RpcResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[allow(clippy::too_many_lines)]
pub async fn rpc_handler(
    req: web::Json<RpcRequest>,
    repo: web::Data<Arc<dyn CddRepository>>,
    cfg: Option<web::Data<AppConfig>>,
    executor: Option<web::Data<Arc<dyn DocsJsonExecutor>>>,
) -> impl Responder {
    if req.jsonrpc != "2.0" {
        return HttpResponse::BadRequest().json(RpcResponse::error(
            req.id.clone(),
            -32600,
            "Invalid Request: jsonrpc must be '2.0'".to_string(),
        ));
    }

    let id = req.id.clone();

    match req.method.as_str() {
        "ping" => HttpResponse::Ok().json(RpcResponse::success(id, json!("pong"))),
        "version" => {
            let mut ecosystem = HashMap::new();
            for tool in ALLOWED_TARGETS {
                ecosystem.insert((*tool).to_string(), "0.0.1".to_string());
            }
            let v = VersionResponse {
                gateway: env!("CARGO_PKG_VERSION").to_string(),
                ecosystem,
            };
            ok_rpc_json(id, serde_json::to_value(&v))
        }
        "to_docs_json" => {
            let params_val = req.params.clone().unwrap_or(Value::Null);
            let Ok(params) = serde_json::from_value::<ToDocsJsonParams>(params_val) else {
                return HttpResponse::BadRequest().json(RpcResponse::error(
                    id,
                    -32602,
                    "Invalid params: 'target_language' and 'input' are required".to_string(),
                ));
            };

            let target = match resolve_target_binary(&params.target_language) {
                Ok(t) => t,
                Err(e) => {
                    return HttpResponse::BadRequest().json(RpcResponse::error(
                        id,
                        -32602,
                        e.to_string(),
                    ));
                }
            };

            let mode = cfg
                .as_ref()
                .map_or(ExecutionMode::Native, |c| c.execution_mode);

            let default_exec = DefaultDocsJsonExecutor::default();
            let exec_ref: &dyn DocsJsonExecutor = match &executor {
                Some(d) => d.get_ref().as_ref(),
                None => &default_exec,
            };

            match exec_ref.execute(&target, &params, mode).await {
                Ok(val) => HttpResponse::Ok().json(RpcResponse::success(id, val)),
                Err(e) => HttpResponse::Ok().json(RpcResponse::error(id, 500, e.to_string())),
            }
        }
        "get_organization" => {
            let params_val = req.params.clone().unwrap_or(Value::Null);
            let Ok(params) = serde_json::from_value::<GetOrgParams>(params_val) else {
                return HttpResponse::BadRequest().json(RpcResponse::error(
                    id,
                    -32602,
                    "Invalid params: 'org_id' (integer) is required".to_string(),
                ));
            };
            match repo.get_organization(params.org_id).await {
                Ok(Some(org)) => ok_rpc_json(id, serde_json::to_value(&org)),
                Ok(None) => HttpResponse::Ok().json(RpcResponse::error(
                    id,
                    404,
                    "Organization not found".to_string(),
                )),
                Err(e) => HttpResponse::InternalServerError().json(RpcResponse::error(
                    id,
                    500,
                    format!("Database error: {e}"),
                )),
            }
        }
        "get_repository" => {
            let params_val = req.params.clone().unwrap_or(Value::Null);
            let Ok(params) = serde_json::from_value::<GetRepoParams>(params_val) else {
                return HttpResponse::BadRequest().json(RpcResponse::error(
                    id,
                    -32602,
                    "Invalid params: 'repo_id' (integer) is required".to_string(),
                ));
            };
            match repo.get_repository(params.repo_id).await {
                Ok(Some(r)) => ok_rpc_json(id, serde_json::to_value(&r)),
                Ok(None) => HttpResponse::Ok().json(RpcResponse::error(
                    id,
                    404,
                    "Repository not found".to_string(),
                )),
                Err(e) => HttpResponse::InternalServerError().json(RpcResponse::error(
                    id,
                    500,
                    format!("Database error: {e}"),
                )),
            }
        }
        "get_user_role" => {
            let params_val = req.params.clone().unwrap_or(Value::Null);
            let Ok(params) = serde_json::from_value::<GetUserRoleParams>(params_val) else {
                return HttpResponse::BadRequest().json(RpcResponse::error(
                    id,
                    -32602,
                    "Invalid params: 'org_id' and 'user_id' (integers) are required".to_string(),
                ));
            };
            match repo.get_user_role(params.org_id, params.user_id).await {
                Ok(Some(role)) => {
                    HttpResponse::Ok().json(RpcResponse::success(id, json!({ "role": role })))
                }
                Ok(None) => HttpResponse::Ok().json(RpcResponse::error(
                    id,
                    404,
                    "User role not found in organization".to_string(),
                )),
                Err(e) => HttpResponse::InternalServerError().json(RpcResponse::error(
                    id,
                    500,
                    format!("Database error: {e}"),
                )),
            }
        }
        "create_organization" => {
            let params_val = req.params.clone().unwrap_or(Value::Null);
            let Ok(params) = serde_json::from_value::<CreateOrgParams>(params_val) else {
                return HttpResponse::BadRequest().json(RpcResponse::error(
                    id,
                    -32602,
                    "Invalid params: 'login' (string) is required".to_string(),
                ));
            };
            match repo
                .create_organization(params.github_id, params.login, params.description)
                .await
            {
                Ok(org) => ok_rpc_json(id, serde_json::to_value(&org)),
                Err(e) => HttpResponse::InternalServerError().json(RpcResponse::error(
                    id,
                    500,
                    format!("Database error: {e}"),
                )),
            }
        }
        "create_repository" => {
            let params_val = req.params.clone().unwrap_or(Value::Null);
            let Ok(params) = serde_json::from_value::<CreateRepoParams>(params_val) else {
                return HttpResponse::BadRequest().json(RpcResponse::error(
                    id,
                    -32602,
                    "Invalid params: 'org_id' (integer) and 'name' (string) are required"
                        .to_string(),
                ));
            };
            match repo
                .create_repository(
                    params.org_id,
                    params.github_id,
                    params.name,
                    params.description,
                )
                .await
            {
                Ok(r) => ok_rpc_json(id, serde_json::to_value(&r)),
                Err(e) => HttpResponse::InternalServerError().json(RpcResponse::error(
                    id,
                    500,
                    format!("Database error: {e}"),
                )),
            }
        }
        _ => HttpResponse::NotFound().json(RpcResponse::error(
            id,
            -32601,
            format!("Method not found: {}", req.method),
        )),
    }
}

/// Registers the `/rpc` route on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/rpc", web::post().to(rpc_handler));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Organization, Repository};
    use crate::db::repository::MockCddRepository;
    use actix_web::{test, App};
    use serde::ser::Error as _;

    struct FailingSerialize;

    impl Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(S::Error::custom("forced failure"))
        }
    }

    #[actix_web::test]
    async fn test_resolve_target_binary() {
        assert_eq!(
            resolve_target_binary("rust").ok(),
            Some("cdd-rust".to_string())
        );
        assert_eq!(
            resolve_target_binary("cdd-go").ok(),
            Some("cdd-go".to_string())
        );
        assert!(resolve_target_binary("").is_err());
        assert!(resolve_target_binary("../bin/sh").is_err());
        assert!(resolve_target_binary("cdd-dir/mock").is_err());
        assert!(resolve_target_binary("cdd-unknown").is_err());
    }

    #[actix_web::test]
    async fn test_ok_rpc_json_serialization_error() {
        let resp = ok_rpc_json(Some(json!(1)), serde_json::to_value(&FailingSerialize));
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[actix_web::test]
    async fn test_rpc_envelope_ping_version_and_unknown() {
        let repo = MockCddRepository::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        // Invalid jsonrpc version
        let req_bad_ver = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "1.0".into(),
                method: "ping".into(),
                params: None,
                id: Some(json!(1)),
            })
            .to_request();
        let resp_bad_ver: RpcResponse = test::call_and_read_body_json(&app, req_bad_ver).await;
        assert!(resp_bad_ver.error.is_some());

        // Ping
        let req_ping = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".into(),
                method: "ping".into(),
                params: None,
                id: Some(json!(1)),
            })
            .to_request();
        let resp_ping: RpcResponse = test::call_and_read_body_json(&app, req_ping).await;
        assert_eq!(resp_ping.result, Some(json!("pong")));

        // Version
        let req_ver = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".into(),
                method: "version".into(),
                params: None,
                id: Some(json!(2)),
            })
            .to_request();
        let resp_ver: RpcResponse = test::call_and_read_body_json(&app, req_ver).await;
        assert!(resp_ver.result.is_some());

        // Unknown method
        let req_unk = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".into(),
                method: "nonexistent".into(),
                params: None,
                id: Some(json!(3)),
            })
            .to_request();
        let resp_unk: RpcResponse = test::call_and_read_body_json(&app, req_unk).await;
        assert_eq!(resp_unk.error.map(|e| e.code), Some(-32601));
    }

    #[actix_web::test]
    async fn test_rpc_to_docs_json_branches() {
        let mut mock_exec = MockDocsJsonExecutor::default();
        mock_exec
            .expect_execute()
            .times(1)
            .returning(|target, params, mode| {
                assert_eq!(target, "cdd-rust");
                assert_eq!(params.input, "spec.json");
                assert_eq!(mode, ExecutionMode::Wasm);
                Ok(json!({ "docs": "ok" }))
            });
        mock_exec
            .expect_execute()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Internal("compiler failed".into())));

        let cfg = AppConfig {
            execution_mode: ExecutionMode::Wasm,
            ..Default::default()
        };

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Arc::new(MockCddRepository::new()) as Arc<dyn CddRepository>
                ))
                .app_data(web::Data::new(cfg))
                .app_data(web::Data::new(
                    Arc::new(mock_exec) as Arc<dyn DocsJsonExecutor>
                ))
                .configure(configure),
        )
        .await;

        // 1. Missing params
        let req_missing = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".into(),
                method: "to_docs_json".into(),
                params: Some(json!({ "input": "spec.json" })),
                id: Some(json!(1)),
            })
            .to_request();
        let resp_missing: RpcResponse = test::call_and_read_body_json(&app, req_missing).await;
        assert_eq!(resp_missing.error.map(|e| e.code), Some(-32602));

        // 2. Disallowed target_language (path traversal attempt)
        let req_bad_target = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".into(),
                method: "to_docs_json".into(),
                params: Some(json!({
                    "target_language": "../bin/sh",
                    "input": "spec.json"
                })),
                id: Some(json!(2)),
            })
            .to_request();
        let resp_bad_target: RpcResponse =
            test::call_and_read_body_json(&app, req_bad_target).await;
        assert_eq!(resp_bad_target.error.map(|e| e.code), Some(-32602));

        // 3. Executor success
        let req_ok = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".into(),
                method: "to_docs_json".into(),
                params: Some(json!({
                    "target_language": "rust",
                    "input": "spec.json",
                    "no_imports": true,
                    "no_wrapping": true
                })),
                id: Some(json!(3)),
            })
            .to_request();
        let resp_ok: RpcResponse = test::call_and_read_body_json(&app, req_ok).await;
        assert_eq!(resp_ok.result, Some(json!({ "docs": "ok" })));

        // 4. Executor error
        let req_err = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".into(),
                method: "to_docs_json".into(),
                params: Some(json!({
                    "target_language": "cdd-rust",
                    "input": "spec.json"
                })),
                id: Some(json!(4)),
            })
            .to_request();
        let resp_err: RpcResponse = test::call_and_read_body_json(&app, req_err).await;
        assert_eq!(resp_err.error.map(|e| e.code), Some(500));

        // 5. Default executor fallback when no executor web::Data is registered
        let app_no_exec = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Arc::new(MockCddRepository::new()) as Arc<dyn CddRepository>
                ))
                .configure(configure),
        )
        .await;
        let req_default_exec = test::TestRequest::post()
            .uri("/rpc")
            .set_json(RpcRequest {
                jsonrpc: "2.0".into(),
                method: "to_docs_json".into(),
                params: Some(json!({
                    "target_language": "cdd-swift",
                    "input": "/nonexistent/spec.json"
                })),
                id: Some(json!(5)),
            })
            .to_request();
        let resp_default_exec: RpcResponse =
            test::call_and_read_body_json(&app_no_exec, req_default_exec).await;
        assert_eq!(resp_default_exec.error.map(|e| e.code), Some(500));
    }

    #[actix_web::test]
    async fn test_default_docs_json_executor_native_and_wasm(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = std::env::temp_dir().join(format!("cdd_exec_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        // 1. Success binary
        let bin_ok = temp_dir.join("cdd-rust");
        {
            let mut f = std::fs::File::create(&bin_ok)?;
            writeln!(f, "#!/bin/sh")?;
            writeln!(f, r#"echo '{{"status":"ok"}}'"#)?;
            #[cfg(unix)]
            std::fs::set_permissions(&bin_ok, std::fs::Permissions::from_mode(0o755))?;
        }

        // 2. Failing exit status binary
        let bin_fail = temp_dir.join("cdd-go");
        {
            let mut f = std::fs::File::create(&bin_fail)?;
            writeln!(
                f,
                "#!/bin/sh
echo 'failed' >&2
exit 1"
            )?;
            #[cfg(unix)]
            std::fs::set_permissions(&bin_fail, std::fs::Permissions::from_mode(0o755))?;
        }

        // 3. Invalid JSON stdout binary (with no_imports/no_wrapping = false)
        let bin_bad_json = temp_dir.join("cdd-c");
        {
            let mut f = std::fs::File::create(&bin_bad_json)?;
            writeln!(
                f,
                "#!/bin/sh
echo 'not-json'"
            )?;
            #[cfg(unix)]
            std::fs::set_permissions(&bin_bad_json, std::fs::Permissions::from_mode(0o755))?;
        }

        let exec = DefaultDocsJsonExecutor {
            bin_dir: Some(temp_dir.clone()),
        };

        let params = ToDocsJsonParams {
            target_language: "rust".into(),
            input: "Cargo.toml".into(),
            no_imports: true,
            no_wrapping: true,
        };
        let params_no_flags = ToDocsJsonParams {
            target_language: "rust".into(),
            input: "Cargo.toml".into(),
            no_imports: false,
            no_wrapping: false,
        };

        let res_ok = exec
            .execute("cdd-rust", &params, ExecutionMode::Native)
            .await?;
        assert_eq!(res_ok, json!({"status": "ok"}));

        assert!(exec
            .execute("cdd-go", &params, ExecutionMode::Native)
            .await
            .is_err());

        assert!(exec
            .execute("cdd-c", &params_no_flags, ExecutionMode::Native)
            .await
            .is_err());

        // Non-existent binary in bin_dir
        assert!(exec
            .execute("cdd-cpp", &params, ExecutionMode::Native)
            .await
            .is_err());

        // Default bin_dir: None path
        let default_exec = DefaultDocsJsonExecutor::default();
        assert!(default_exec
            .execute("cdd-swift", &params, ExecutionMode::Native)
            .await
            .is_err());

        // WASM execution path (non-existent input triggers error cleanly)
        let bad_wasm_params = ToDocsJsonParams {
            target_language: "rust".into(),
            input: "/nonexistent/spec.json".into(),
            no_imports: true,
            no_wrapping: true,
        };
        assert!(default_exec
            .execute("cdd-rust", &bad_wasm_params, ExecutionMode::Wasm)
            .await
            .is_err());
        let bad_wasm_no_flags = ToDocsJsonParams {
            target_language: "rust".into(),
            input: "/nonexistent/spec.json".into(),
            no_imports: false,
            no_wrapping: false,
        };
        assert!(default_exec
            .execute("cdd-rust", &bad_wasm_no_flags, ExecutionMode::Wasm)
            .await
            .is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[actix_web::test]
    async fn test_rpc_database_methods() {
        let mut repo = MockCddRepository::new();

        // get_organization: Ok(Some), Ok(None), Err
        repo.expect_get_organization().times(1).returning(|_| {
            Ok(Some(Organization {
                id: 1,
                github_id: None,
                login: "org1".into(),
                description: None,
            }))
        });
        repo.expect_get_organization()
            .times(1)
            .returning(|_| Ok(None));
        repo.expect_get_organization()
            .times(1)
            .returning(|_| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        // get_repository: Ok(Some), Ok(None), Err
        repo.expect_get_repository().times(1).returning(|_| {
            Ok(Some(Repository {
                id: 1,
                organization_id: 1,
                github_id: None,
                name: "repo1".into(),
                description: None,
            }))
        });
        repo.expect_get_repository()
            .times(1)
            .returning(|_| Ok(None));
        repo.expect_get_repository()
            .times(1)
            .returning(|_| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        // get_user_role: Ok(Some), Ok(None), Err
        repo.expect_get_user_role()
            .times(1)
            .returning(|_, _| Ok(Some("owner".into())));
        repo.expect_get_user_role()
            .times(1)
            .returning(|_, _| Ok(None));
        repo.expect_get_user_role()
            .times(1)
            .returning(|_, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        // create_organization: Ok, Err
        repo.expect_create_organization()
            .times(1)
            .returning(|_, login, _| {
                Ok(Organization {
                    id: 1,
                    github_id: None,
                    login,
                    description: None,
                })
            });
        repo.expect_create_organization()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        // create_repository: Ok, Err
        repo.expect_create_repository()
            .times(1)
            .returning(|org_id, _, name, _| {
                Ok(Repository {
                    id: 1,
                    organization_id: org_id,
                    github_id: None,
                    name,
                    description: None,
                })
            });
        repo.expect_create_repository()
            .times(1)
            .returning(|_, _, _, _| {
                Err(CddGatewayError::Database(diesel::result::Error::NotFound))
            });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let call_rpc = |method: &str, params: Value| {
            test::TestRequest::post()
                .uri("/rpc")
                .set_json(RpcRequest {
                    jsonrpc: "2.0".into(),
                    method: method.into(),
                    params: Some(params),
                    id: Some(json!(1)),
                })
                .to_request()
        };

        // get_organization: invalid params, found, not found, db err
        let r: RpcResponse =
            test::call_and_read_body_json(&app, call_rpc("get_organization", json!({}))).await;
        assert_eq!(r.error.map(|e| e.code), Some(-32602));
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_organization", json!({ "org_id": 1 })),
        )
        .await;
        assert!(r.result.is_some());
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_organization", json!({ "org_id": 2 })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(404));
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_organization", json!({ "org_id": 3 })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(500));

        // get_repository: invalid params, found, not found, db err
        let r: RpcResponse =
            test::call_and_read_body_json(&app, call_rpc("get_repository", json!({}))).await;
        assert_eq!(r.error.map(|e| e.code), Some(-32602));
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_repository", json!({ "repo_id": 1 })),
        )
        .await;
        assert!(r.result.is_some());
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_repository", json!({ "repo_id": 2 })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(404));
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_repository", json!({ "repo_id": 3 })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(500));

        // get_user_role: invalid params, found, not found, db err
        let r: RpcResponse =
            test::call_and_read_body_json(&app, call_rpc("get_user_role", json!({ "org_id": 1 })))
                .await;
        assert_eq!(r.error.map(|e| e.code), Some(-32602));
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_user_role", json!({ "org_id": 1, "user_id": 1 })),
        )
        .await;
        assert!(r.result.is_some());
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_user_role", json!({ "org_id": 1, "user_id": 2 })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(404));
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("get_user_role", json!({ "org_id": 1, "user_id": 3 })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(500));

        // create_organization: invalid params, ok, db err
        let r: RpcResponse =
            test::call_and_read_body_json(&app, call_rpc("create_organization", json!({}))).await;
        assert_eq!(r.error.map(|e| e.code), Some(-32602));
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("create_organization", json!({ "login": "org1" })),
        )
        .await;
        assert!(r.result.is_some());
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("create_organization", json!({ "login": "org1" })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(500));

        // create_repository: invalid params, ok, db err
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("create_repository", json!({ "org_id": 1 })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(-32602));
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("create_repository", json!({ "org_id": 1, "name": "r1" })),
        )
        .await;
        assert!(r.result.is_some());
        let r: RpcResponse = test::call_and_read_body_json(
            &app,
            call_rpc("create_repository", json!({ "org_id": 1, "name": "r1" })),
        )
        .await;
        assert_eq!(r.error.map(|e| e.code), Some(500));
    }
}
