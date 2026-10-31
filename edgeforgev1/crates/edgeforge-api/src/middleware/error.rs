// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use edgeforge_core::EdgeForgeError;
use serde_json::json;
pub struct ApiError(pub EdgeForgeError);
impl<E: Into<EdgeForgeError>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = Json(
            json!({ "error": { "type": error_type(&self.0), "message": self.0.to_string() } }),
        );
        (status, body).into_response()
    }
}
fn error_type(e: &EdgeForgeError) -> &'static str {
    match e {
        EdgeForgeError::InvalidRequest(_) | EdgeForgeError::InvalidPreset(_) => "invalid_request",
        EdgeForgeError::Unauthorized(_) => "unauthorized",
        EdgeForgeError::RateLimited(_) => "rate_limited",
        EdgeForgeError::ModelNotFound(_) | EdgeForgeError::JobNotFound(_) => "not_found",
        EdgeForgeError::NotImplemented(_) => "not_implemented",
        _ => "internal_error",
    }
}
