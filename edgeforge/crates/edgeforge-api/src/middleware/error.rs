//! # Centralized Error → JSON Response Mapping
//!
//! ## Purpose
//! Converts EdgeForgeError into a JSON HTTP response with the correct status
//! code and error type.
//!
//! ## What This File Does
//! - `ApiError(EdgeForgeError)` — wrapper struct
//! - `From<E: Into<EdgeForgeError>>` — automatic conversion
//! - `IntoResponse` — maps to (StatusCode, Json({"error":{...}}))
//! - `error_type()` — maps EdgeForgeError variant to error type string
//!
//! ## Dependencies
//! - `axum::{StatusCode, Json, IntoResponse}`
//! - `edgeforge_core::EdgeForgeError`
//! - `serde_json::json`
//!
//! ## Output
//! HTTP response with JSON body: `{"error":{"type":"...","message":"..."}}`
//!
//! ## Error Type Mapping
//! | EdgeForgeError | Error Type |
//! |---|---|
//! | InvalidRequest, InvalidPreset | invalid_request |
//! | Unauthorized | unauthorized |
//! | RateLimited | rate_limited |
//! | ModelNotFound, JobNotFound | not_found |
//! | NotImplemented | not_implemented |
//! | Others | internal_error |

use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use edgeforge_core::EdgeForgeError;
use serde_json::json;
/// `ApiError` — see module documentation for purpose and usage.
pub struct ApiError(pub EdgeForgeError);
impl<E: Into<EdgeForgeError>> From<E> for ApiError { fn from(e: E) -> Self { Self(e.into()) } }
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.0.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = Json(json!({ "error": { "type": error_type(&self.0), "message": self.0.to_string() } }));
        (status, body).into_response()
    }
}
fn error_type(e: &EdgeForgeError) -> &'static str { match e { EdgeForgeError::InvalidRequest(_) | EdgeForgeError::InvalidPreset(_) => "invalid_request", EdgeForgeError::Unauthorized(_) => "unauthorized", EdgeForgeError::RateLimited(_) => "rate_limited", EdgeForgeError::ModelNotFound(_) | EdgeForgeError::JobNotFound(_) => "not_found", EdgeForgeError::NotImplemented(_) => "not_implemented", _ => "internal_error" } }

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_invalid_request_maps_to_400() {
        let err = ApiError(EdgeForgeError::InvalidRequest("bad".into()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_model_not_found_maps_to_404() {
        let err = ApiError(EdgeForgeError::ModelNotFound("01J9".into()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_not_implemented_maps_to_501() {
        let err = ApiError(EdgeForgeError::NotImplemented("stub"));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[test]
    fn test_rate_limited_maps_to_429() {
        let err = ApiError(EdgeForgeError::RateLimited("too fast".into()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_engine_error_maps_to_500() {
        let err = ApiError(EdgeForgeError::Engine("crash".into()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_error_type_invalid_request() {
        assert_eq!(error_type(&EdgeForgeError::InvalidRequest("x".into())), "invalid_request");
    }

    #[test]
    fn test_error_type_not_found() {
        assert_eq!(error_type(&EdgeForgeError::ModelNotFound("x".into())), "not_found");
    }
}
