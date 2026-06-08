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
