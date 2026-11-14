use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::models::UserRecord;
use crate::domain::validation::{normalize_email, validate_account};
use crate::domain::{now_iso, TERMS_VERSION};
use crate::error::AppError;
use crate::security::{hash_password, issue_token, password_matches};
use crate::state::AppState;

use super::{blocking, require_active, AuthUser};

#[derive(Debug, Deserialize)]
struct RegisterBody {
    email: String,
    password: String,
    full_name: String,
    phone: String,
    document: String,
    role: String,
}

#[derive(Debug, Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct StatusBody {
    status: String,
}

#[derive(Debug, Deserialize)]
struct UserPath {
    id: String,
}

#[post("/auth/register")]
pub async fn register(
    state: web::Data<AppState>,
    body: web::Json<RegisterBody>,
) -> Result<HttpResponse, AppError> {
    let account = validate_account(
        &body.email,
        &body.password,
        &body.full_name,
        &body.phone,
        &body.document,
        &body.role,
    )?;
    let repo = state.repo.clone();
    let secret = state.jwt_secret.clone();
    let ttl = state.token_ttl_hours;
    let (public_user, token) = blocking(move || {
        if repo.find_user_by_email(&account.email)?.is_some() {
            return Err(AppError::Conflict("e-mail já cadastrado".into()));
        }
        let now = now_iso();
        let record = UserRecord {
            id: Uuid::new_v4().to_string(),
            email: account.email,
            password_hash: hash_password(&account.password)?,
            full_name: account.full_name,
            phone: account.phone,
            document: account.document,
            role: account.role,
            status: "active".into(),
            terms_version: TERMS_VERSION.into(),
            consent_at: now.clone(),
            created_at: now.clone(),
            updated_at: now,
            last_login_at: None,
        };
        let public_user = record.to_public();
        let user_id = record.id.clone();
        let role = record.role.clone();
        repo.create_user(record)?;
        let token = issue_token(&secret, &user_id, &role, ttl)?;
        Ok((public_user, token))
    })
    .await?;
    Ok(session(true, token, ttl, public_user))
}

#[post("/auth/login")]
pub async fn login(
    state: web::Data<AppState>,
    body: web::Json<LoginBody>,
) -> Result<HttpResponse, AppError> {
    let email = normalize_email(&body.email);
    let password = body.password.clone();
    let repo = state.repo.clone();
    let secret = state.jwt_secret.clone();
    let ttl = state.token_ttl_hours;
    let (public_user, token) = blocking(move || {
        let user = repo.find_user_by_email(&email)?;
        let hash = user.as_ref().map(|item| item.password_hash.as_str());
        if !password_matches(&password, hash) {
            return Err(AppError::Unauthorized);
        }
        let user = user.ok_or(AppError::Unauthorized)?;
        if user.status != "active" {
            return Err(AppError::Forbidden("conta suspensa".into()));
        }
        let now = now_iso();
        repo.set_last_login(&user.id, &now)?;
        let token = issue_token(&secret, &user.id, &user.role, ttl)?;
        Ok((user.to_public(), token))
    })
    .await?;
    Ok(session(false, token, ttl, public_user))
}

#[get("/me")]
pub async fn me(state: web::Data<AppState>, user: AuthUser) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let public_user =
        blocking(move || Ok(require_active(repo.as_ref(), &user_id)?.to_public())).await?;
    Ok(HttpResponse::Ok().json(public_user))
}

#[post("/users/{id}/status")]
pub async fn change_status(
    state: web::Data<AppState>,
    actor: AuthUser,
    path: web::Path<UserPath>,
    body: web::Json<StatusBody>,
) -> Result<HttpResponse, AppError> {
    if actor.role != "admin" {
        return Err(AppError::Forbidden(
            "somente a equipe Wattly altera o status de uma conta".into(),
        ));
    }
    if !matches!(body.status.as_str(), "active" | "suspended") {
        return Err(AppError::field("status", "use active ou suspended"));
    }
    if actor.id == path.id {
        return Err(AppError::Conflict(
            "a própria conta administradora não pode ser alterada por aqui".into(),
        ));
    }
    let status = body.status.clone();
    let target = path.id.clone();
    let repo = state.repo.clone();
    let actor_id = actor.id.clone();
    let public_user = blocking(move || {
        require_active(repo.as_ref(), &actor_id)?;
        let user = repo
            .find_user_by_id(&target)?
            .ok_or_else(|| AppError::NotFound("usuário não encontrado".into()))?;
        repo.set_user_status(&user.id, &status, &now_iso())?;
        let mut updated = user;
        updated.status = status;
        Ok(updated.to_public())
    })
    .await?;
    Ok(HttpResponse::Ok().json(public_user))
}

fn session(
    created: bool,
    token: String,
    ttl: i64,
    user: crate::domain::models::PublicUser,
) -> HttpResponse {
    let body = json!({
        "token_type": "Bearer",
        "expires_in_hours": ttl,
        "token": token,
        "user": user
    });
    if created {
        HttpResponse::Created().json(body)
    } else {
        HttpResponse::Ok().json(body)
    }
}
