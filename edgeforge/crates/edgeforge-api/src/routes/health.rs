//! # GET /health — Liveness Probe
//!
//! ## Purpose
//! Returns server status and version. Used by Docker healthcheck and
//! load balancers.
//!
//! ## What This File Does
//! - `health()` — returns HealthResponse with status="ok" and version
//!
//! ## Dependencies
//! - `axum` — routing
//!
//! ## Output
//! `Json<HealthResponse>` — `{"status":"ok","version":"0.1.0"}`

use crate::dto::HealthResponse;
use axum::{routing::get, Json, Router};
/// Returns the axum Router for this module's routes.
pub fn routes() -> Router<crate::AppState> { Router::new().route("/health", get(health)) }
async fn health() -> Json<HealthResponse> { Json(HealthResponse { status: "ok", version: env!("CARGO_PKG_VERSION") }) }

#[cfg(test)]
mod tests {
    use crate::dto::HealthResponse;

    #[test]
    fn test_health_response_fields() {
        let resp = HealthResponse { status: "ok", version: "0.1.0" };
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.version, "0.1.0");
    }

    #[test]
    fn test_health_response_serializes() {
        let resp = HealthResponse { status: "ok", version: "0.1.0" };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("ok"));
        assert!(json.contains("0.1.0"));
    }

    #[test]
    fn test_cargo_pkg_version_is_nonempty() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
