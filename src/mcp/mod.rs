#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Server-Sent Events (SSE) and JSON-RPC message transport for the Model Context Protocol (MCP).

use crate::error::CddGatewayError;
use actix_web::{web, HttpResponse, Responder};
use cdd_engine::mcp::{McpOrchestrator, McpRequest};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use utoipa::ToSchema;

/// Typed arguments for the MCP `generate_sdk` tool call.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct McpGenerateSdkArguments {
    /// Target language identifier (e.g., `"rust"`, `"python-all"`, `"cdd-go"`).
    pub language: String,
    /// `OpenAPI`, Swagger 2.0, Arazzo 1.1.0, or Google Discovery JSON specification payload.
    pub spec_content: String,
    /// Optional input specification format (`"openapi"`, `"swagger"`, `"arazzo"`, `"google_discovery"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_format: Option<String>,
    /// Optional generator subcommand target (`"to_sdk"`, `"to_sdk_cli"`, `"to_server"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Optional flag enabling Model Context Protocol (`--mcp`) generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<bool>,
    /// Optional framework name (`--framework <name>`, e.g. `"angular"`, `"actix"`, `"fastapi"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    /// Optional ORM name (`--orm <name>`, e.g. `"diesel"`, `"sqlalchemy"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orm: Option<String>,
}

/// Canonical list of allowed input specification formats for MCP `generate_sdk`.
const ALLOWED_MCP_SPEC_FORMATS: &[&str] = &["openapi", "swagger", "arazzo", "google_discovery"];

/// Canonical list of allowed generator subcommand targets for MCP `generate_sdk`.
const ALLOWED_MCP_TARGETS: &[&str] = &["to_sdk", "to_sdk_cli", "to_server", "to_docs_json"];

impl McpGenerateSdkArguments {
    /// Validates `spec_format` and `target` on [`McpGenerateSdkArguments`].
    ///
    /// # Arguments
    /// * `&self` - The arguments instance to validate.
    ///
    /// # Returns
    /// * `Ok(())` - If all optional fields are valid.
    ///
    /// # Errors
    /// * [`CddGatewayError::Validation`] - If `spec_format` or `target` is outside the allowlist.
    pub fn validate(&self) -> Result<(), CddGatewayError> {
        if let Some(fmt) = &self.spec_format {
            if !ALLOWED_MCP_SPEC_FORMATS.contains(&fmt.as_str()) {
                return Err(CddGatewayError::Validation(format!(
                    "Unsupported MCP spec_format: {fmt}"
                )));
            }
        }
        if let Some(tgt) = &self.target {
            if !ALLOWED_MCP_TARGETS.contains(&tgt.as_str()) {
                return Err(CddGatewayError::Validation(format!(
                    "Unsupported MCP target: {tgt}"
                )));
            }
        }
        Ok(())
    }

    /// Builds the additional CLI flags vector forwarded to `cdd-engine` for `generate_sdk`.
    ///
    /// # Arguments
    /// * `&self` - The validated arguments instance.
    ///
    /// # Returns
    /// * `Vec<String>` - Ordered CLI arguments for `cdd-<language>`.
    #[must_use]
    pub fn build_extra_args(&self) -> Vec<String> {
        let subcommand = self.target.as_deref().unwrap_or("to_sdk");
        let mut args = vec![subcommand.to_string()];
        if let Some(fmt) = &self.spec_format {
            args.push("--spec-format".to_string());
            args.push(fmt.clone());
        }
        if self.mcp == Some(true) {
            args.push("--mcp".to_string());
        }
        if let Some(fw) = &self.framework {
            args.push("--framework".to_string());
            args.push(fw.clone());
        }
        if let Some(orm) = &self.orm {
            args.push("--orm".to_string());
            args.push(orm.clone());
        }
        args
    }
}

/// Validates and enriches a `tools/call` MCP request targeting `generate_sdk`.
///
/// # Arguments
/// * `req` - Incoming [`McpRequest`] envelope.
///
/// # Returns
/// * `Ok(McpRequest)` - Validated and enriched MCP request ready for `McpOrchestrator`.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `generate_sdk` arguments contain an unsupported `spec_format` or `target`.
fn normalize_mcp_request(mut req: McpRequest) -> Result<McpRequest, CddGatewayError> {
    if req.method != "tools/call" {
        return Ok(req);
    }
    let Some(params) = req.params.as_mut() else {
        return Ok(req);
    };
    if params.get("name").and_then(serde_json::Value::as_str) != Some("generate_sdk") {
        return Ok(req);
    }
    let Some(args_obj) = params
        .get_mut("arguments")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(req);
    };
    let Ok(parsed_args) = serde_json::from_value::<McpGenerateSdkArguments>(
        serde_json::Value::Object(args_obj.clone()),
    ) else {
        return Ok(req);
    };
    parsed_args.validate()?;
    let extra_args = parsed_args.build_extra_args();
    args_obj.insert("cli_args".to_string(), serde_json::json!(extra_args));
    Ok(req)
}

/// Initiates an MCP Server-Sent Events (SSE) session (`GET /mcp/sse`).
///
/// Emits the `endpoint` discovery event pointing clients to `/mcp/message`.
///
/// # Returns
/// * `impl Responder` - `200 OK` `text/event-stream` response yielding the initial MCP endpoint event.
#[allow(clippy::unused_async)]
pub async fn mcp_sse_handshake() -> impl Responder {
    let (tx, rx) = mpsc::channel(10);

    let init_event = "event: endpoint
data: /mcp/message

";
    let _ = tx.try_send(Ok::<_, actix_web::Error>(web::Bytes::from(init_event)));

    let stream = ReceiverStream::new(rx);

    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(stream)
}

/// Processes an incoming MCP JSON-RPC message (`POST /mcp/message`).
///
/// Delegates execution to the configured [`McpOrchestrator`].
///
/// # Arguments
/// * `req` - Deserialized [`McpRequest`] JSON payload.
/// * `orchestrator` - Shared [`McpOrchestrator`] engine handle.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` JSON response containing the orchestrator's [`cdd_engine::mcp::McpResponse`].
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `generate_sdk` arguments fail validation.
/// * [`CddGatewayError::Engine`] - If the [`McpOrchestrator`] returns an execution error.
pub async fn mcp_message_handler(
    req: web::Json<McpRequest>,
    orchestrator: web::Data<Arc<dyn McpOrchestrator>>,
) -> Result<HttpResponse, CddGatewayError> {
    let normalized = normalize_mcp_request(req.into_inner())?;
    let response = orchestrator.handle_request(normalized).await?;
    Ok(HttpResponse::Ok().json(response))
}

/// Registers the `/mcp` scope routes (`/mcp/sse` and `/mcp/message`) on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/mcp")
            .route("/sse", web::get().to(mcp_sse_handshake))
            .route("/message", web::post().to(mcp_message_handler)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use async_trait::async_trait;
    use cdd_engine::error::CddEngineError;
    use cdd_engine::mcp::McpResponse;
    use serde_json::json;

    struct DummyOrchestrator;

    #[async_trait]
    impl McpOrchestrator for DummyOrchestrator {
        async fn handle_request(&self, req: McpRequest) -> Result<McpResponse, CddEngineError> {
            if req.method == "error" {
                Err(CddEngineError::Internal("Simulated error".into()))
            } else {
                Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(json!({"echo": req.method})),
                    error: None,
                })
            }
        }
    }

    #[actix_web::test]
    async fn test_mcp_generate_sdk_arguments_validation() -> Result<(), CddGatewayError> {
        for fmt in &["openapi", "swagger", "arazzo", "google_discovery"] {
            for tgt in &["to_sdk", "to_sdk_cli", "to_server", "to_docs_json"] {
                let args = McpGenerateSdkArguments {
                    language: "rust".into(),
                    spec_content: "{}".into(),
                    spec_format: Some((*fmt).to_string()),
                    target: Some((*tgt).to_string()),
                    mcp: Some(true),
                    framework: Some("actix".into()),
                    orm: Some("diesel".into()),
                };
                args.validate()?;
                let cli = args.build_extra_args();
                assert_eq!(cli[0], *tgt);
                assert!(cli.contains(&"--spec-format".to_string()));
                assert!(cli.contains(&"--mcp".to_string()));
                assert!(cli.contains(&"--framework".to_string()));
                assert!(cli.contains(&"--orm".to_string()));
            }
        }

        let default_args = McpGenerateSdkArguments {
            language: "rust".into(),
            spec_content: "{}".into(),
            ..Default::default()
        };
        default_args.validate()?;
        assert_eq!(default_args.build_extra_args(), vec!["to_sdk".to_string()]);

        let bad_fmt = McpGenerateSdkArguments {
            spec_format: Some("raml".into()),
            ..Default::default()
        };
        assert!(bad_fmt.validate().is_err());

        let bad_tgt = McpGenerateSdkArguments {
            target: Some("exec".into()),
            ..Default::default()
        };
        assert!(bad_tgt.validate().is_err());

        // Test normalize_mcp_request edge branches (no params, other tool name, missing arguments, unparseable arguments)
        let r_no_params = normalize_mcp_request(McpRequest {
            jsonrpc: "2.0".into(),
            id: None,
            method: "tools/call".into(),
            params: None,
        })?;
        assert!(r_no_params.params.is_none());

        let r_other_tool = normalize_mcp_request(McpRequest {
            jsonrpc: "2.0".into(),
            id: None,
            method: "tools/call".into(),
            params: Some(json!({ "name": "other_tool" })),
        })?;
        assert!(r_other_tool.params.is_some());

        let r_no_args = normalize_mcp_request(McpRequest {
            jsonrpc: "2.0".into(),
            id: None,
            method: "tools/call".into(),
            params: Some(json!({ "name": "generate_sdk" })),
        })?;
        assert!(r_no_args.params.is_some());

        let r_bad_args = normalize_mcp_request(McpRequest {
            jsonrpc: "2.0".into(),
            id: None,
            method: "tools/call".into(),
            params: Some(json!({ "name": "generate_sdk", "arguments": 123 })),
        })?;
        assert!(r_bad_args.params.is_some());

        let r_incomplete_args = normalize_mcp_request(McpRequest {
            jsonrpc: "2.0".into(),
            id: None,
            method: "tools/call".into(),
            params: Some(json!({ "name": "generate_sdk", "arguments": { "language": "rust" } })),
        })?;
        assert!(r_incomplete_args.params.is_some());

        Ok(())
    }

    #[actix_web::test]
    async fn test_mcp_sse_handshake() {
        let app = test::init_service(App::new().configure(configure)).await;
        let req = test::TestRequest::get().uri("/mcp/sse").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        assert_eq!(
            resp.headers()
                .get("Content-Type")
                .and_then(|v| v.to_str().ok()),
            Some("text/event-stream")
        );
        let body = test::read_body(resp).await;
        assert_eq!(
            body,
            "event: endpoint
data: /mcp/message

"
        );
    }

    #[actix_web::test]
    async fn test_mcp_message_handler() {
        let orchestrator: Arc<dyn McpOrchestrator> = Arc::new(DummyOrchestrator);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(orchestrator))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/mcp/message")
            .set_json(json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "ping"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Valid generate_sdk arguments with spec_format and --mcp forwarded
        let req_gen_valid = test::TestRequest::post()
            .uri("/mcp/message")
            .set_json(json!({
                "jsonrpc": "2.0",
                "id": "10",
                "method": "tools/call",
                "params": {
                    "name": "generate_sdk",
                    "arguments": {
                        "language": "rust",
                        "spec_content": r#"{"openapi":"3.0.0"}"#,
                        "spec_format": "openapi",
                        "target": "to_sdk",
                        "mcp": true,
                        "framework": "actix",
                        "orm": "diesel"
                    }
                }
            }))
            .to_request();
        let resp_gen_valid = test::call_service(&app, req_gen_valid).await;
        assert!(resp_gen_valid.status().is_success());

        // Invalid spec_format in generate_sdk returns 400 Bad Request
        let req_gen_bad_fmt = test::TestRequest::post()
            .uri("/mcp/message")
            .set_json(json!({
                "jsonrpc": "2.0",
                "id": "11",
                "method": "tools/call",
                "params": {
                    "name": "generate_sdk",
                    "arguments": {
                        "language": "rust",
                        "spec_content": "{}",
                        "spec_format": "unknown_spec"
                    }
                }
            }))
            .to_request();
        let resp_gen_bad_fmt = test::call_service(&app, req_gen_bad_fmt).await;
        assert_eq!(resp_gen_bad_fmt.status(), 400);

        let req_err = test::TestRequest::post()
            .uri("/mcp/message")
            .set_json(json!({
                "jsonrpc": "2.0",
                "id": "2",
                "method": "error"
            }))
            .to_request();
        let resp_err = test::call_service(&app, req_err).await;
        assert!(resp_err.status().is_server_error());
    }
}
