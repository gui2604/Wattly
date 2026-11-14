use std::sync::Arc;

use actix_web::{middleware::Logger, web, App, HttpServer};
use wattly_api::bootstrap::ensure_admin;
use wattly_api::config::Config;
use wattly_api::http::{cors, json_config, routes, security_headers};
use wattly_api::repo::oracle::OracleRepo;
use wattly_api::repo::Repository;
use wattly_api::state::AppState;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    load_env();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wattly_api=info".into()),
        )
        .init();

    let config = Config::from_env().unwrap_or_else(|message| {
        eprintln!("configuração inválida: {message}");
        std::process::exit(1);
    });
    let bind = config.bind.clone();
    let repo = OracleRepo::connect(&config).unwrap_or_else(|err| {
        eprintln!("não foi possível abrir o Oracle: {err}");
        std::process::exit(1);
    });
    let repo: Arc<dyn Repository> = Arc::new(repo);
    if let Err(err) = ensure_admin(repo.as_ref(), &config) {
        eprintln!("falha ao preparar o administrador: {err}");
        std::process::exit(1);
    }

    let state = AppState {
        repo,
        jwt_secret: Arc::from(config.jwt_secret),
        token_ttl_hours: config.token_ttl_hours,
    };
    tracing::info!(%bind, "Wattly API escutando");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(json_config())
            .wrap(security_headers())
            .wrap(cors())
            .wrap(Logger::new("%a \"%r\" %s %Dms"))
            .configure(routes)
    })
    .bind(&bind)?
    .run()
    .await
}

fn load_env() {
    for path in [".env", "api/.env", "../.env"] {
        let _ = dotenvy::from_filename(path);
    }
}
