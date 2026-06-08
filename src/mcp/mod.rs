#![deny(missing_docs)]
//! Server-Sent Events (SSE) transport for Model Context Protocol.

use crate::error::CddGatewayError;
use actix_web::{web, HttpResponse, Responder};
use cdd_engine::mcp::{McpOrchestrator, McpRequest};
use log::info;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Initialize the SSE stream for a new MCP client.
pub async fn mcp_sse_handshake() -> impl Responder {
    let (tx, rx) = mpsc::channel::<Result<actix_web::web::Bytes, actix_web::Error>>(10);

    // According to MCP spec, the server should send an initial endpoint event
    let init_msg = "event: endpoint\ndata: /mcp/message\n\n".to_string();
    let _ = tx.try_send(Ok(actix_web::web::Bytes::from(init_msg)));

    // Return the streaming response
    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// Handle incoming POST messages on the MCP transport.
pub async fn mcp_message_handler(
    engine: web::Data<Arc<dyn McpOrchestrator>>,
    req: web::Json<McpRequest>,
) -> Result<impl Responder, CddGatewayError> {
    info!("Received MCP Request: {} for {}", req.jsonrpc, req.method);

    let response = engine
        .handle_request(req.into_inner())
        .await
        .map_err(CddGatewayError::Engine)?;

    Ok(HttpResponse::Ok().json(response))
}

/// Configure the MCP routes in Actix.
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
    use actix_web::{test, web, App};
    use async_trait::async_trait;
    use cdd_engine::mcp::McpResponse;
    use serde_json::json;

    struct MockEngine;

    #[async_trait]
    impl McpOrchestrator for MockEngine {
        async fn handle_request(
            &self,
            req: McpRequest,
        ) -> Result<McpResponse, cdd_engine::error::CddEngineError> {
            if req.method == "error" {
                return Err(cdd_engine::error::CddEngineError::Internal(
                    "test error".to_string(),
                ));
            }
            Ok(McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(json!({"status": "ok"})),
                error: None,
            })
        }
    }

    #[actix_web::test]
    async fn test_mcp_sse_handshake() {
        let req = test::TestRequest::default().to_http_request();
        let resp = mcp_sse_handshake().await.respond_to(&req);
        assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
        assert_eq!(
            resp.headers().get("Content-Type").unwrap(),
            "text/event-stream"
        );
    }

    #[actix_web::test]
    async fn test_mcp_message_handler() {
        let engine: Arc<dyn McpOrchestrator> = Arc::new(MockEngine);
        let req_data = serde_json::from_value(json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": "test"
        }))
        .unwrap();
        let req = web::Json(req_data);

        let resp = mcp_message_handler(web::Data::new(engine), req)
            .await
            .unwrap();
        let http_req = test::TestRequest::default().to_http_request();
        assert_eq!(
            resp.respond_to(&http_req).status(),
            actix_web::http::StatusCode::OK
        );
    }

    #[actix_web::test]
    async fn test_configure() {
        let engine: Arc<dyn McpOrchestrator> = Arc::new(MockEngine);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(engine.clone()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::get().uri("/mcp/sse").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let req = test::TestRequest::post()
            .uri("/mcp/message")
            .set_json(json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "test"
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
