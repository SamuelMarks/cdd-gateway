#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Server-Sent Events (SSE) and JSON-RPC message transport for the Model Context Protocol (MCP).

use crate::error::CddGatewayError;
use actix_web::{web, HttpResponse, Responder};
use cdd_engine::mcp::{McpOrchestrator, McpRequest};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

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
/// * [`CddGatewayError::Engine`] - If the [`McpOrchestrator`] returns an execution error.
pub async fn mcp_message_handler(
    req: web::Json<McpRequest>,
    orchestrator: web::Data<Arc<dyn McpOrchestrator>>,
) -> Result<HttpResponse, CddGatewayError> {
    let response = orchestrator.handle_request(req.into_inner()).await?;
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
