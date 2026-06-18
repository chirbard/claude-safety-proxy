use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use bytes::Bytes;
use tracing::debug;

const UPSTREAM: &str = "https://api.anthropic.com";

pub async fn forward(
    req: Request<Body>,
    body_bytes: Bytes,
    client: &reqwest::Client,
) -> Result<Response<Body>> {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let url = format!("{UPSTREAM}{path_and_query}");
    debug!("Forwarding {} {}", req.method(), url);

    // Build reqwest request
    let mut builder = client.request(req.method().clone(), &url);

    // Copy headers, replacing Host
    for (name, value) in req.headers() {
        if name == axum::http::header::HOST {
            continue;
        }
        builder = builder.header(name, value);
    }

    let upstream_resp = builder
        .body(body_bytes)
        .send()
        .await?;

    let status = StatusCode::from_u16(upstream_resp.status().as_u16())?;

    let mut response_builder = Response::builder().status(status);

    for (name, value) in upstream_resp.headers() {
        response_builder = response_builder.header(name, value);
    }

    // Stream body back
    let stream = upstream_resp.bytes_stream();
    let body = Body::from_stream(stream);

    Ok(response_builder.body(body)?)
}
