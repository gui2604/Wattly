use std::future::{ready, Ready};

use actix_web::{dev::Payload, http::header::AUTHORIZATION, web, FromRequest, HttpRequest};

use crate::error::AppError;
use crate::security::read_token;
use crate::state::AppState;

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    pub role: String,
}

pub struct MaybeUser(pub Option<AuthUser>);

impl FromRequest for AuthUser {
    type Error = AppError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(identity(req, true).map(|user| user.expect("token obrigatório")))
    }
}

impl FromRequest for MaybeUser {
    type Error = AppError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(identity(req, false).map(MaybeUser))
    }
}

fn identity(req: &HttpRequest, required: bool) -> Result<Option<AuthUser>, AppError> {
    let header = req.headers().get(AUTHORIZATION);
    if header.is_none() {
        return if required {
            Err(AppError::Unauthorized)
        } else {
            Ok(None)
        };
    }
    let value = header
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or(AppError::Unauthorized)?;
    let state = req
        .app_data::<web::Data<AppState>>()
        .ok_or(AppError::Internal)?;
    let identity = read_token(&state.jwt_secret, token)?;
    Ok(Some(AuthUser {
        id: identity.user_id,
        role: identity.role,
    }))
}
