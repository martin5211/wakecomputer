use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("machine not found: {0}")]
    MachineNotFound(String),

    #[error("WoL error: {0}")]
    Wol(String),

    #[error("agent error: {0}")]
    Agent(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!(error = %self, "request failed");

        let status = match &self {
            AppError::MachineNotFound(_) => StatusCode::NOT_FOUND,
            AppError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, axum::Json(json!({ "error": self.to_string() }))).into_response()
    }
}
