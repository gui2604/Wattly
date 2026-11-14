use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::http::StatusCode;
use actix_web::test::{self, TestRequest};
use actix_web::{web, App};
use serde_json::{json, Value};
use uuid::Uuid;
use wattly_api::domain::models::UserRecord;
use wattly_api::domain::{now_iso, TERMS_VERSION};
use wattly_api::http::{cors, json_config, routes, security_headers};
use wattly_api::security::hash_password;
use wattly_api::AppState;

const SECRET: &str = "segredo-de-teste-com-32-caracteres";

async fn service(
    state: AppState,
) -> impl Service<
    actix_http::Request,
    Response = ServiceResponse<impl MessageBody>,
    Error = actix_web::Error,
> {
    test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(json_config())
            .wrap(security_headers())
            .wrap(cors())
            .configure(routes),
    )
    .await
}

async fn call<S, B>(app: &S, req: actix_http::Request) -> (StatusCode, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let response = test::call_service(app, req).await;
    let status = response.status();
    let body = test::read_body(response).await;
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

fn bearer(token: &str) -> (&'static str, String) {
    ("authorization", format!("Bearer {token}"))
}

#[actix_web::test]
async fn health_responde_ok() {
    let app = service(AppState::memory(SECRET)).await;
    let (status, body) = call(&app, TestRequest::get().uri("/health").to_request()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "wattly-api");
}

#[actix_web::test]
async fn cadastro_rejeita_cpf_invalido_e_senha_fraca() {
    let app = service(AppState::memory(SECRET)).await;
    let (status, body) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(account("12345678901", "Recarga-2026"))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["fields"].to_string().contains("document"));

    let (status, body) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(account("52998224725", "fraca"))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["fields"].to_string().contains("password"));
}

#[actix_web::test]
async fn cadastro_conflita_quando_o_email_se_repete() {
    let app = service(AppState::memory(SECRET)).await;
    let first = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(account("52998224725", "Recarga-2026"))
            .to_request(),
    )
    .await;
    assert_eq!(first.0, StatusCode::CREATED);
    assert!(first.1["user"]["document_masked"]
        .as_str()
        .unwrap()
        .ends_with("25"));
    assert!(!first.1["user"]["document_masked"]
        .as_str()
        .unwrap()
        .contains("529982247"));

    let second = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(account("52998224725", "Recarga-2026"))
            .to_request(),
    )
    .await;
    assert_eq!(second.0, StatusCode::CONFLICT);
}

#[actix_web::test]
async fn login_rejeita_senha_errada() {
    let app = service(AppState::memory(SECRET)).await;
    call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(account("52998224725", "Recarga-2026"))
            .to_request(),
    )
    .await;
    let (status, _) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({"email": "ana@wattly.dev", "password": "senha-errada-1"}))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn fluxo_de_credenciamento_aprova_e_esconde_o_endereco() {
    let state = AppState::memory(SECRET);
    seed_admin(&state);
    let app = service(state).await;

    let (status, host) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(account("52998224725", "Recarga-2026"))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let host_token = host["token"].as_str().unwrap();

    let (status, _) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/clients/nao-e-uuid/review")
            .insert_header(bearer(host_token))
            .set_json(json!({"action": "approve"}))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, client) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/clients")
            .insert_header(bearer(host_token))
            .set_json(json!({
                "trade_name": "Garagem da Ana",
                "person_type": "individual",
                "tax_id": "529.982.247-25",
                "pix_key_type": "email",
                "pix_key": "ana@wattly.dev",
                "bio": "Vaga coberta na Vila Mariana"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{client}");
    let client_id = client["id"].as_str().unwrap();
    assert!(client["tax_id"].as_str().unwrap().contains("52998224725"));
    assert_eq!(client["tax_id_masked"], "***.***.***-25");

    let (status, _) = call(
        &app,
        TestRequest::post()
            .uri(&format!("/api/v1/clients/{client_id}/locations"))
            .insert_header(bearer(host_token))
            .set_json(location_body(0))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, location) = call(
        &app,
        TestRequest::post()
            .uri(&format!("/api/v1/clients/{client_id}/locations"))
            .insert_header(bearer(host_token))
            .set_json(location_body(40))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{location}");
    let location_id = location["id"].as_str().unwrap();
    assert_eq!(location["address_line"], "Rua das Acacias");
    assert!((location["latitude"].as_f64().unwrap() - -23.561414).abs() < 0.000_001);

    let (status, _) = call(
        &app,
        TestRequest::post()
            .uri(&format!("/api/v1/clients/{client_id}/submit"))
            .insert_header(bearer(host_token))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    for kind in ["identity", "address_proof", "charger_photo"] {
        let (status, _) = call(
            &app,
            TestRequest::post()
                .uri(&format!("/api/v1/clients/{client_id}/documents"))
                .insert_header(bearer(host_token))
                .set_json(json!({
                    "kind": kind,
                    "storage_ref": format!("local://documentos/{kind}.pdf")
                }))
                .to_request(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (status, submitted) = call(
        &app,
        TestRequest::post()
            .uri(&format!("/api/v1/clients/{client_id}/submit"))
            .insert_header(bearer(host_token))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(submitted["status"], "submitted");

    let (status, admin) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({"email": "admin@wattly.dev", "password": "Admin-2026"}))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{admin}");
    let admin_token = admin["token"].as_str().unwrap();

    let (status, _) = call(
        &app,
        TestRequest::post()
            .uri(&format!("/api/v1/clients/{client_id}/review"))
            .insert_header(bearer(admin_token))
            .set_json(json!({"action": "approve"}))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    call(
        &app,
        TestRequest::post()
            .uri(&format!("/api/v1/clients/{client_id}/review"))
            .insert_header(bearer(admin_token))
            .set_json(json!({"action": "start"}))
            .to_request(),
    )
    .await;
    let (status, approved) = call(
        &app,
        TestRequest::post()
            .uri(&format!("/api/v1/clients/{client_id}/review"))
            .insert_header(bearer(admin_token))
            .set_json(json!({
                "action": "approve",
                "notes": "Documentos e instalação conferidos",
                "checks": [
                    {"item": "identity", "passed": true},
                    {"item": "address", "passed": true},
                    {"item": "electrical", "passed": true},
                    {"item": "connector", "passed": true}
                ]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["status"], "approved");

    let (status, search) = call(
        &app,
        TestRequest::get()
            .uri("/api/v1/locations?lat=-23.56&lng=-46.66&radius_km=5")
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let rendered = search.to_string();
    assert!(rendered.contains("Garagem da Ana"), "{search}");
    assert!(!rendered.contains("Rua das Acacias"), "{search}");
    assert!(!rendered.contains("Interfone"), "{search}");
    assert_eq!(search["locations"][0]["latitude"], -23.56);
    assert_eq!(search["locations"][0]["approximate"], true);

    let (status, exact) = call(
        &app,
        TestRequest::get()
            .uri(&format!("/api/v1/locations/{location_id}"))
            .insert_header(bearer(host_token))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exact}");
    assert_eq!(exact["address_line"], "Rua das Acacias");
    assert_eq!(exact["approximate"], false);

    let (status, dossier) = call(
        &app,
        TestRequest::get()
            .uri(&format!("/api/v1/clients/{client_id}/dossier"))
            .insert_header(bearer(host_token))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(dossier["events"].as_array().unwrap().len() >= 5);
    assert_eq!(dossier["checks"].as_array().unwrap().len(), 4);
}

#[actix_web::test]
async fn usuario_suspenso_nao_entra() {
    let state = AppState::memory(SECRET);
    seed_admin(&state);
    let app = service(state).await;
    let (status, host) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/register")
            .set_json(account("52998224725", "Recarga-2026"))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let host_id = host["user"]["id"].as_str().unwrap();
    let (status, admin) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({"email": "admin@wattly.dev", "password": "Admin-2026"}))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let admin_token = admin["token"].as_str().unwrap();
    let (status, _) = call(
        &app,
        TestRequest::post()
            .uri(&format!("/api/v1/users/{host_id}/status"))
            .insert_header(bearer(admin_token))
            .set_json(json!({"status": "suspended"}))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &app,
        TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({"email": "ana@wattly.dev", "password": "Recarga-2026"}))
            .to_request(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

fn account(document: &str, password: &str) -> Value {
    json!({
        "email": "ana@wattly.dev",
        "password": password,
        "full_name": "Ana Souza",
        "phone": "11987654321",
        "document": document,
        "role": "host"
    })
}

fn location_body(price_per_minute_cents: i64) -> Value {
    json!({
        "label": "Garagem da Ana",
        "description": "Vaga coberta",
        "address_line": "Rua das Acacias",
        "address_number": "120",
        "district": "Vila Mariana",
        "city": "Sao Paulo",
        "state": "SP",
        "postal_code": "04101000",
        "latitude": -23.561414,
        "longitude": -46.655881,
        "connector": "home_outlet_220v",
        "power_kw": 7.4,
        "voltage": 220,
        "phases": 1,
        "price_per_minute_cents": price_per_minute_cents,
        "price_per_hour_cents": 1800,
        "install_type": "home_outlet",
        "has_roof": true,
        "well_lit": true,
        "access_note": "Interfone 12",
        "spot_code": "Vaga 2",
        "windows": [{"weekday": 1, "start": "08:00", "end": "18:00"}]
    })
}

fn seed_admin(state: &AppState) {
    let now = now_iso();
    state
        .repo
        .create_user(UserRecord {
            id: Uuid::new_v4().to_string(),
            email: "admin@wattly.dev".into(),
            password_hash: hash_password("Admin-2026").expect("hash"),
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
        .expect("admin");
}
