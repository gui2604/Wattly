use uuid::Uuid;

use crate::config::Config;
use crate::domain::models::UserRecord;
use crate::domain::validation::normalize_email;
use crate::domain::{now_iso, TERMS_VERSION};
use crate::error::AppError;
use crate::repo::Repository;
use crate::security::hash_password;

pub fn ensure_admin(repo: &dyn Repository, config: &Config) -> Result<(), AppError> {
    let (Some(email), Some(password)) = (
        config.admin_email.as_deref(),
        config.admin_password.as_deref(),
    ) else {
        return Ok(());
    };
    let email = normalize_email(email);
    if repo.find_user_by_email(&email)?.is_some() {
        return Ok(());
    }
    let now = now_iso();
    repo.create_user(UserRecord {
        id: Uuid::new_v4().to_string(),
        email,
        password_hash: hash_password(password)?,
        full_name: "Equipe Wattly".into(),
        phone: "11000000000".into(),
        document: "52998224725".into(),
        role: "admin".into(),
        status: "active".into(),
        terms_version: TERMS_VERSION.into(),
        consent_at: now.clone(),
        created_at: now.clone(),
        updated_at: now,
        last_login_at: None,
    })
}
