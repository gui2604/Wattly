use actix_web::{get, web, HttpResponse};
use serde_json::json;

use crate::error::AppError;
use crate::state::AppState;

use super::blocking;

#[get("/health")]
pub async fn live() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "status": "ok",
        "service": "wattly-api"
    }))
}

#[get("/health/ready")]
pub async fn ready(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let database = blocking(move || repo.ping()).await?;
    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "database": database
    })))
}
