use crate::domain::validation::validate_password;
use crate::error::AppError;

#[derive(Clone)]
pub struct Config {
    pub bind: String,
    pub oracle_user: String,
    pub oracle_password: String,
    pub oracle_connect: String,
    pub jwt_secret: String,
    pub token_ttl_hours: i64,
    pub admin_email: Option<String>,
    pub admin_password: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let oracle_user = required("ORACLE_USER")?;
        let oracle_password = required("ORACLE_PASSWORD")?;
        let oracle_connect = required("ORACLE_CONNECT_STRING")?;
        let jwt_secret = required("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            return Err("JWT_SECRET precisa ter pelo menos 32 caracteres".into());
        }

        let admin_email = optional("WATTLY_ADMIN_EMAIL");
        let admin_password = optional("WATTLY_ADMIN_PASSWORD");
        match (&admin_email, &admin_password) {
            (Some(_), None) | (None, Some(_)) => {
                return Err(
                    "WATTLY_ADMIN_EMAIL e WATTLY_ADMIN_PASSWORD precisam ser definidos juntos"
                        .into(),
                );
            }
            (Some(_), Some(password)) => {
                if let Err(err) = validate_password(password) {
                    return Err(format!("senha do administrador inválida: {err}"));
                }
            }
            (None, None) => {}
        }

        let token_ttl_hours = optional("WATTLY_TOKEN_TTL_HOURS")
            .unwrap_or_else(|| "12".into())
            .parse::<i64>()
            .map_err(|_| "WATTLY_TOKEN_TTL_HOURS precisa ser um número inteiro".to_string())?;
        if !(1..=168).contains(&token_ttl_hours) {
            return Err("WATTLY_TOKEN_TTL_HOURS precisa estar entre 1 e 168".into());
        }

        Ok(Self {
            bind: optional("WATTLY_BIND").unwrap_or_else(|| "0.0.0.0:8080".into()),
            oracle_user,
            oracle_password,
            oracle_connect,
            jwt_secret,
            token_ttl_hours,
            admin_email,
            admin_password,
        })
    }
}

fn required(name: &str) -> Result<String, String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(format!("{name} não está definido")),
    }
}

fn optional(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

impl std::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Config")
            .field("bind", &self.bind)
            .field("oracle_user", &self.oracle_user)
            .field("oracle_password", &"***")
            .field("oracle_connect", &self.oracle_connect)
            .field("jwt_secret", &"***")
            .field("token_ttl_hours", &self.token_ttl_hours)
            .field("admin_email", &self.admin_email)
            .field("admin_password", &"***")
            .finish()
    }
}

pub fn database(err: impl std::fmt::Display) -> AppError {
    let rendered = err.to_string();
    let safe = if let Ok(password) = std::env::var("ORACLE_PASSWORD") {
        rendered.replace(&password, "***")
    } else {
        rendered
    };
    tracing::error!(error = %safe, "falha de banco");
    AppError::Database
}
