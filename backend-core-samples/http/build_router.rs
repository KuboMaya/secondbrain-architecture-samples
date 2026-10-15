//! Portfolio sample — Axum router composition.
//! Highlights: domain routers merged under `/api`, Tailscale access middleware, optional PWA static fallback.
//! Source: extracted from `src-tauri/src/lib.rs` (`build_router` + listener wiring).

use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::core::AppState;
use crate::domains;
use crate::sync;

/// Compose the HTTP surface: each domain owns its `router()`, then share one access gate.
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

    // In production, an optional `www/` SPA fallback enables Tailscale Serve same-origin hosting.
    // Omitted here for brevity — see full `lib.rs`.

    router
        .layer(middleware::from_fn_with_state(
            state.clone(),
            sync::access::sync_access_middleware,
        ))
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Domain listeners are spawned once at boot — HTTP handlers stay thin (0ms path).
///
/// ```ignore
/// domains::reward::event_listener::spawn(pool.clone(), event_bus.clone());
/// domains::ai::event_listener::spawn(pool.clone(), event_bus.clone(), gemini_key);
/// domains::sleep::event_listener::spawn(/* … voice server url … */);
/// // … other domains …
/// ```
pub fn note_on_event_listeners() {}
