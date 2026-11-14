use std::sync::OnceLock;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    role: String,
    iss: String,
    exp: usize,
}

pub struct Identity {
    pub user_id: String,
    pub role: String,
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| {
            tracing::error!(error = %err, "falha ao gerar hash de senha");
            AppError::Internal
        })
}

pub fn verify_password(password: &str, encoded: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(encoded) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password("wattly-dummy-password").unwrap_or_else(|_| {
            "$argon2id$v=19$m=19456,t=2,p=1$d2F0dGx5ZHVtbXlzYWx0$invalid".into()
        })
    })
}

/// Confere a senha mesmo quando o usuário não existe, para o tempo de resposta
/// não revelar se o e-mail está cadastrado.
pub fn password_matches(password: &str, encoded: Option<&str>) -> bool {
    match encoded {
        Some(hash) => verify_password(password, hash),
        None => {
            let _ = verify_password(password, dummy_hash());
            false
        }
    }
}

pub fn issue_token(
    secret: &str,
    user_id: &str,
    role: &str,
    ttl_hours: i64,
) -> Result<String, AppError> {
    let exp = (Utc::now() + Duration::hours(ttl_hours)).timestamp();
    if exp < 0 {
        return Err(AppError::Internal);
    }
    let claims = Claims {
        sub: user_id.to_string(),
        role: role.to_string(),
        iss: "wattly".to_string(),
        exp: exp as usize,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|err| {
        tracing::error!(error = %err, "falha ao emitir token");
        AppError::Internal
    })
}

pub fn read_token(secret: &str, token: &str) -> Result<Identity, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&["wattly"]);
    validation.validate_exp = true;
    validation.leeway = 30;
    let token = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Unauthorized)?;
    Ok(Identity {
        user_id: token.claims.sub,
        role: token.claims.role,
    })
}

#[cfg(test)]
mod tests {
    use super::{hash_password, issue_token, password_matches, read_token, verify_password};

    #[test]
    fn senha_correta_confere_e_senha_errada_nao() {
        let hash = hash_password("Recarga-2026").expect("hash");
        assert!(verify_password("Recarga-2026", &hash));
        assert!(!verify_password("outra-senha", &hash));
        assert!(!hash.contains("Recarga-2026"));
    }

    #[test]
    fn usuario_inexistente_nao_autentica() {
        assert!(!password_matches("Recarga-2026", None));
    }

    #[test]
    fn token_emitido_volta_com_o_mesmo_usuario() {
        let secret = "segredo-de-teste-com-32-caracteres";
        let token = issue_token(secret, "user-1", "host", 2).expect("token");
        let identity = read_token(secret, &token).expect("identity");
        assert_eq!(identity.user_id, "user-1");
        assert_eq!(identity.role, "host");
        assert!(read_token("outro-segredo-com-32-caracteres-ok", &token).is_err());
    }
}
