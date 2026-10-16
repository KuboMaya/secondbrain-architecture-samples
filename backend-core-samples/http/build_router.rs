//! Portfolio sample — Axum router + tower-http layers + access middleware.
//! Highlights: nest `/api`, CORS, trace, body limit, Tailscale Bearer gate.
//! Source: condensed from `src-tauri/src/lib.rs` + `sync/access.rs` wiring.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue, Method};
use axum::middleware;
use axum::Router;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::core::error::AppResult;
use crate::core::AppState;
use crate::domains;
use crate::sync;

/// Compose HTTP surface. Domain routers stay thin; cross-cutting concerns are layers.
pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .merge(domains::task::handlers::router())
        .merge(domains::reward::handlers::router())
        .merge(domains::knowledge::handlers::router())
        .merge(domains::inventory::handlers::router())
        .merge(domains::sleep::handlers::router())
        .merge(domains::weather::handlers::router())
        .merge(domains::ai::handlers::router())
        .merge(domains::device::handlers::router())
        .merge(sync::engine::router());

    let router = Router::new().nest("/api", api);

    router
        .layer(middleware::from_fn_with_state(
            state.clone(),
            sync::access::sync_access_middleware,
        ))
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// CORS for Tailscale Serve / PWA same-origin and local Vite dev.
/// Production often uses `CorsLayer::permissive()` behind Tailscale; this sample
/// shows an explicit allow-list style for analyzers / review.
fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:5173"),
            // Placeholder for Tailscale Serve host (replace in real deploy).
            HeaderValue::from_static("https://example.ts.net"),
        ]))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
        ])
        .allow_credentials(true)
}

/// Boot: bind Axum with connect-info so middleware can read peer IP.
pub async fn start_axum_server(state: AppState, bind_addr: &str) -> AppResult<()> {
    // Placeholder bind — real config uses env / private secrets file.
    let addr = if bind_addr.is_empty() {
        "0.0.0.0:3000"
    } else {
        bind_addr
    };
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| crate::core::error::AppError::Internal(format!("bind {addr}: {err}")))?;

    tracing::info!(%addr, "axum listening (portfolio sample)");

    axum::serve(
        listener,
        build_router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|err| crate::core::error::AppError::Internal(format!("server error: {err}")))?;
    Ok(())
}

/// Spawn domain EventBus listeners once at process start (HTTP path stays 0ms).
pub fn spawn_domain_listeners(state: &AppState) {
    let pool = state.pool.clone();
    let bus = state.event_bus.clone();
    let gemini_key = state
        .config
        .gemini_api_key
        .clone()
        .or_else(|| Some("dummy_api_key".into()));

    domains::reward::event_listener::spawn(pool.clone(), bus.clone());
    domains::ai::event_listener::spawn(pool.clone(), bus.clone(), gemini_key);
    domains::task::event_listener::spawn(pool.clone(), bus.clone());
    domains::sleep::event_listener::spawn(
        pool.clone(),
        bus.clone(),
        Some("/path/to/tts-stub.exe".into()),
        Some("http://127.0.0.1:8787".into()),
        state.http.clone(),
    );
    let _ = Arc::strong_count(&state.pool);
}
