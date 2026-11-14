use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    Validation {
        message: String,
        fields: Vec<(String, String)>,
    },
    #[error("credenciais inválidas")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("falha ao acessar os dados")]
    Database,
    #[error("erro interno")]
    Internal,
}

impl AppError {
    pub fn validation(message: impl Into<String>, fields: Vec<(String, String)>) -> Self {
        Self::Validation {
            message: message.into(),
            fields,
        }
    }

    pub fn field(field: &str, detail: impl Into<String>) -> Self {
        Self::validation(
            "há campos inválidos na requisição",
            vec![(field.to_string(), detail.into())],
        )
    }

    fn body(&self) -> serde_json::Value {
        match self {
            Self::Validation { message, fields } => json!({
                "error": "validation",
                "message": message,
                "fields": fields
                    .iter()
                    .map(|(field, detail)| json!({ "field": field, "detail": detail }))
                    .collect::<Vec<_>>()
            }),
            Self::Unauthorized => json!({
                "error": "unauthorized",
                "message": "credenciais inválidas"
            }),
            Self::Forbidden(message) => json!({
                "error": "forbidden",
                "message": message
            }),
            Self::NotFound(message) => json!({
                "error": "not_found",
                "message": message
            }),
            Self::Conflict(message) => json!({
                "error": "conflict",
                "message": message
            }),
            Self::Database => json!({
                "error": "database",
                "message": "não foi possível concluir a operação de dados"
            }),
            Self::Internal => json!({
                "error": "internal",
                "message": "erro interno"
            }),
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Validation { .. } => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Database | Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(self.body())
    }
}
