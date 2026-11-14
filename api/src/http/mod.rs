mod auth;
mod clients;
mod extract;
mod health;
mod locations;
mod openapi;

use actix_cors::Cors;
use actix_web::{
    middleware::DefaultHeaders,
    web::{self, ServiceConfig},
};

use crate::error::AppError;
use crate::repo::Repository;

pub use extract::{AuthUser, MaybeUser};

pub fn json_config() -> web::JsonConfig {
    web::JsonConfig::default()
        .limit(65_536)
        .error_handler(|err, _| {
            AppError::validation(
                "o corpo da requisição não é um JSON válido",
                vec![("body".into(), err.to_string())],
            )
            .into()
        })
}

pub fn security_headers() -> DefaultHeaders {
    DefaultHeaders::new()
        .add(("X-Content-Type-Options", "nosniff"))
        .add(("Referrer-Policy", "no-referrer"))
        .add(("Cache-Control", "no-store"))
        .add(("X-Frame-Options", "DENY"))
}

pub fn cors() -> Cors {
    Cors::default()
        .allow_any_origin()
        .allow_any_method()
        .allow_any_header()
        .max_age(3600)
}

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.service(health::live)
        .service(health::ready)
        .service(openapi::specification)
        .service(openapi::docs)
        .service(
            web::scope("/api/v1")
                .service(auth::register)
                .service(auth::login)
                .service(auth::me)
                .service(auth::change_status)
                .service(clients::create_client)
                .service(clients::list_clients)
                .service(clients::get_client)
                .service(clients::update_client)
                .service(clients::dossier)
                .service(clients::add_document)
                .service(clients::submit_client)
                .service(clients::reopen_client)
                .service(clients::review_client)
                .service(clients::create_location)
                .service(clients::list_client_locations)
                .service(locations::search_locations)
                .service(locations::get_location),
        );
}

pub async fn blocking<T, F>(work: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    match web::block(work).await {
        Ok(result) => result,
        Err(err) => {
            tracing::error!(error = %err, "tarefa bloqueante");
            Err(AppError::Internal)
        }
    }
}

pub fn require_active(
    repo: &dyn Repository,
    user_id: &str,
) -> Result<crate::domain::models::UserRecord, AppError> {
    let user = repo
        .find_user_by_id(user_id)?
        .ok_or(AppError::Unauthorized)?;
    if user.status != "active" {
        return Err(AppError::Forbidden("conta suspensa".into()));
    }
    Ok(user)
}
