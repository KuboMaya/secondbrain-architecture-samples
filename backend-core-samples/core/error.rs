//! Portfolio sample — Axum `IntoResponse` error mapping (409 sync payload included).
//! Source: `src-tauri/src/core/error.rs` (logic unchanged; no secrets).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    /// Sync 409: both local and peer copies for the client to resolve.
    #[error("sync conflict: {message}")]
    SyncConflict {
        message: String,
        local: Value,
        cloud: Value,
    },
    #[error("validation: {0}")]
    Validation(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("{0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::SyncConflict {
                message,
                local,
                cloud,
            } => (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "conflict",
                    "message": message,
                    "local": local,
                    "cloud": cloud,
                })),
            )
                .into_response(),
            other => {
                let status = match &other {
                    Self::NotFound(_) => StatusCode::NOT_FOUND,
                    Self::Conflict(_) => StatusCode::CONFLICT,
                    Self::Validation(_) => StatusCode::BAD_REQUEST,
                    Self::Database(_) | Self::Migration(_) | Self::Internal(_) => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                    Self::SyncConflict { .. } => unreachable!(),
                };

                let message = match &other {
                    Self::Database(err) => {
                        tracing::error!("database error: {err}");
                        "internal server error".to_string()
                    }
                    Self::Migration(err) => {
                        tracing::error!("migration error: {err}");
                        "internal server error".to_string()
                    }
                    other => other.to_string(),
                };

                (status, Json(json!({ "error": message }))).into_response()
            }
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
