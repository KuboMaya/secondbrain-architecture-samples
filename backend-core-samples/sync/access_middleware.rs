//! Portfolio sample — multi-layer sync access control.
//! Highlights: Tailscale CGNAT detection, constant-time-ish Bearer compare, deny public LAN.
//! Source: `src-tauri/src/sync/access.rs` (comments sanitized; no private paths).

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::core::AppState;

/// CGNAT range used by Tailscale: 100.64.0.0/10
pub fn is_tailscale_cgnat(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 100 && (o[1] & 0xC0) == 0x40
        }
        IpAddr::V6(_) => false,
    }
}

pub fn is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

fn bearer_matches(req: &Request, expected: &str) -> bool {
    let Some(header) = req.headers().get(header::AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = header.to_str() else {
        return false;
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return false;
    };
    // Constant-time-ish compare for short personal tokens.
    token.as_bytes().len() == expected.as_bytes().len()
        && token
            .as_bytes()
            .iter()
            .zip(expected.as_bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// Allow: loopback always.
/// Allow: Tailscale CGNAT when `SYNC_AUTH_TOKEN` Bearer matches
///        (token unset → deny remote peers).
/// Deny: everything else (public internet / arbitrary LAN).
pub async fn sync_access_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());

    // Missing connect info (in-process / tests): treat as trusted.
    let Some(ip) = peer else {
        return Ok(next.run(req).await);
    };

    if is_loopback(ip) {
        return Ok(next.run(req).await);
    }

    if !is_tailscale_cgnat(ip) {
        tracing::warn!(%ip, "rejected non-Tailscale peer");
        return Err(StatusCode::FORBIDDEN);
    }

    let Some(token) = state
        .config
        .sync_auth_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        tracing::warn!(%ip, "Tailscale peer rejected: SYNC_AUTH_TOKEN not configured");
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !bearer_matches(&req, token) {
        tracing::warn!(%ip, "Tailscale peer rejected: bad or missing Bearer token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn detects_tailscale_range() {
        assert!(is_tailscale_cgnat(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))));
        assert!(is_tailscale_cgnat(IpAddr::V4(Ipv4Addr::new(100, 127, 255, 255))));
        assert!(!is_tailscale_cgnat(IpAddr::V4(Ipv4Addr::new(100, 63, 255, 255))));
        assert!(!is_tailscale_cgnat(IpAddr::V4(Ipv4Addr::new(100, 128, 0, 1))));
        assert!(!is_tailscale_cgnat(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_loopback(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    }
}
