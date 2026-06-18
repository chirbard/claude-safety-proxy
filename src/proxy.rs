use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;
use std::sync::Arc;
use tracing::{info, warn};

use crate::config::Config;
use crate::extractor;
use crate::forwarder;
use crate::scanner;

pub struct AppState {
    pub config: Config,
    pub client: reqwest::Client,
}

pub async fn handle(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    let path = uri.path();

    // Buffer the request body
    let (parts, body) = req.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            warn!("Failed to read request body: {e}");
            return error_response(StatusCode::BAD_GATEWAY, "Failed to read request body");
        }
    };

    info!("{method} {path} ({} bytes)", body_bytes.len());

    // Only scan POST /v1/messages
    if method == axum::http::Method::POST && path == "/v1/messages" {
        let texts = extractor::extract_text(&body_bytes);
        info!("Extracted {} text chunk(s) for scanning", texts.len());

        match scanner::scan(&texts, &state.config).await {
            Ok(result) if result.flagged => {
                warn!("Request BLOCKED – {} finding(s)", result.findings.len());
                for f in &result.findings {
                    warn!("  [{category}] {desc}", category = f.category, desc = f.description);
                }
                return block_response(&result.findings);
            }
            Ok(_) => {
                info!("Scan passed – forwarding request");
            }
            Err(e) => {
                warn!("Scanner error (forwarding anyway): {e}");
            }
        }
    }

    // Reconstruct request and forward
    let mut rebuilt = Request::builder()
        .method(parts.method)
        .uri(parts.uri)
        .version(parts.version);

    for (name, value) in &headers {
        rebuilt = rebuilt.header(name, value);
    }

    let req_to_forward = rebuilt.body(Body::empty()).unwrap();

    match forwarder::forward(req_to_forward, body_bytes, &state.client).await {
        Ok(resp) => resp,
        Err(e) => {
            warn!("Forward error: {e}");
            error_response(StatusCode::BAD_GATEWAY, &format!("Upstream error: {e}"))
        }
    }
}

fn block_response(findings: &[crate::scanner::Finding]) -> Response {
    let details: Vec<String> = findings
        .iter()
        .map(|f| format!("[{}] {}", f.category, f.description))
        .collect();

    let message = format!(
        "🚫 Safety proxy blocked this request — sensitive content detected:\n\n{}\n\nRemove the flagged content and try again.",
        details.join("\n")
    );

    // Use Anthropic's error envelope so Claude Code renders the message directly
    let body = serde_json::json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": message
        }
    });
    (StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({
        "type": "error",
        "error": {
            "type": "api_error",
            "message": message
        }
    });
    (status, axum::Json(body)).into_response()
}
