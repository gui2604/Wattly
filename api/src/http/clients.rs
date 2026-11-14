use actix_web::{get, patch, post, web, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::credential::{
    self, location_status_for, transition, validate_checks, validate_ready, ReviewAction,
};
use crate::domain::models::{
    CheckRecord, ClientRecord, DocumentRecord, Dossier, EventRecord, LocationRecord, WindowRecord,
};
use crate::domain::now_iso;
use crate::domain::validation::{
    document_kind, validate_client, validate_location, LocationInput, NewClient, WindowInput,
};
use crate::error::AppError;
use crate::state::AppState;

use super::{blocking, require_active, AuthUser};

#[derive(Debug, Deserialize)]
struct ClientBody {
    trade_name: String,
    legal_name: Option<String>,
    person_type: String,
    tax_id: String,
    pix_key_type: String,
    pix_key: String,
    bio: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PatchBody {
    trade_name: Option<String>,
    legal_name: Option<String>,
    bio: Option<String>,
    pix_key_type: Option<String>,
    pix_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DocumentBody {
    kind: String,
    storage_ref: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReviewBody {
    action: String,
    notes: Option<String>,
    checks: Option<Vec<CheckBody>>,
}

#[derive(Debug, Clone, Deserialize)]
struct CheckBody {
    item: String,
    passed: bool,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WindowBody {
    weekday: i32,
    start: String,
    end: String,
}

#[derive(Debug, Deserialize)]
struct LocationBody {
    label: String,
    description: Option<String>,
    address_line: String,
    address_number: String,
    complement: Option<String>,
    district: String,
    city: String,
    state: String,
    postal_code: String,
    country: Option<String>,
    latitude: f64,
    longitude: f64,
    connector: String,
    power_kw: f64,
    voltage: i32,
    phases: i32,
    price_per_minute_cents: i64,
    price_per_hour_cents: i64,
    install_type: String,
    has_roof: bool,
    well_lit: bool,
    access_note: Option<String>,
    spot_code: Option<String>,
    windows: Vec<WindowBody>,
}

#[derive(Debug, Deserialize)]
struct ClientPath {
    id: String,
}

#[post("/clients")]
pub async fn create_client(
    state: web::Data<AppState>,
    user: AuthUser,
    body: web::Json<ClientBody>,
) -> Result<HttpResponse, AppError> {
    let draft = validate_client(
        &body.trade_name,
        body.legal_name.as_deref(),
        &body.person_type,
        &body.tax_id,
        &body.pix_key_type,
        &body.pix_key,
        body.bio.as_deref().unwrap_or(""),
    )?;
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let view = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let now = now_iso();
        let client = client_from_draft(draft, user_id, now);
        let view = client.to_view(true);
        repo.create_client(client.clone())?;
        repo.add_event(make_event(
            &client.id,
            &client.owner_id,
            "client_created",
            "none",
            &client.status,
            "{}",
            &client.created_at,
        ))?;
        Ok(view)
    })
    .await?;
    Ok(HttpResponse::Created().json(view))
}

#[get("/clients")]
pub async fn list_clients(
    state: web::Data<AppState>,
    user: AuthUser,
) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let role = user.role.clone();
    let clients = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let records = if role == "admin" {
            repo.list_clients()?
        } else {
            repo.list_clients_by_owner(&user_id)?
        };
        Ok(records
            .iter()
            .map(|client| client.to_view(true))
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(HttpResponse::Ok().json(json!({ "clients": clients })))
}

#[get("/clients/{id}")]
pub async fn get_client(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let role = user.role.clone();
    let client_id = path.id.clone();
    let view = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let client = load_visible(&*repo, &client_id, &user_id, &role)?;
        Ok(client.to_view(true))
    })
    .await?;
    Ok(HttpResponse::Ok().json(view))
}

#[patch("/clients/{id}")]
pub async fn update_client(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
    body: web::Json<PatchBody>,
) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let client_id = path.id.clone();
    let patch = body.into_inner();
    let view = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let mut client = load_owned(&*repo, &client_id, &user_id)?;
        if !client.is_editable() {
            return Err(AppError::Conflict(
                "o credenciamento só pode ser editado em rascunho ou depois de uma recusa".into(),
            ));
        }
        let draft = validate_client(
            patch.trade_name.as_deref().unwrap_or(&client.trade_name),
            Some(patch.legal_name.as_deref().unwrap_or(&client.legal_name)),
            &client.person_type,
            &client.tax_id,
            patch
                .pix_key_type
                .as_deref()
                .unwrap_or(&client.pix_key_type),
            patch.pix_key.as_deref().unwrap_or(&client.pix_key),
            patch.bio.as_deref().unwrap_or(&client.bio),
        )?;
        let now = now_iso();
        client.trade_name = draft.trade_name;
        client.legal_name = draft.legal_name;
        client.pix_key_type = draft.pix_key_type;
        client.pix_key = draft.pix_key;
        client.bio = draft.bio;
        client.updated_at = now.clone();
        let view = client.to_view(true);
        repo.update_client(client.clone())?;
        repo.add_event(make_event(
            &client.id,
            &user_id,
            "client_updated",
            &client.status,
            &client.status,
            "{}",
            &now,
        ))?;
        Ok(view)
    })
    .await?;
    Ok(HttpResponse::Ok().json(view))
}

#[get("/clients/{id}/dossier")]
pub async fn dossier(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let role = user.role.clone();
    let client_id = path.id.clone();
    let dossier = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let client = load_visible(&*repo, &client_id, &user_id, &role)?;
        let documents = repo.list_documents(&client.id)?;
        let checks = repo.list_checks(&client.id)?;
        let events = repo.list_events(&client.id)?;
        let locations = repo
            .list_locations_by_client(&client.id)?
            .into_iter()
            .map(|(location, windows)| location.to_view(&windows, true))
            .collect();
        Ok(Dossier {
            client: client.to_view(true),
            documents,
            checks,
            events,
            locations,
        })
    })
    .await?;
    Ok(HttpResponse::Ok().json(dossier))
}

#[post("/clients/{id}/documents")]
pub async fn add_document(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
    body: web::Json<DocumentBody>,
) -> Result<HttpResponse, AppError> {
    let kind = document_kind(&body.kind)?;
    let storage_ref = body.storage_ref.trim();
    if storage_ref.is_empty() || storage_ref.chars().count() > 300 || storage_ref.contains('\n') {
        return Err(AppError::field(
            "storage_ref",
            "informe uma referência de até 300 caracteres",
        ));
    }
    let note = body
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(300).collect::<String>());
    let storage_ref = storage_ref.to_string();
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let client_id = path.id.clone();
    let document = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let client = load_owned(&*repo, &client_id, &user_id)?;
        if !client.is_editable() {
            return Err(AppError::Conflict(
                "documentos só entram enquanto o credenciamento está aberto".into(),
            ));
        }
        let now = now_iso();
        let document = DocumentRecord {
            id: Uuid::new_v4().to_string(),
            client_id: client.id.clone(),
            kind: kind.to_string(),
            storage_ref,
            status: "received".into(),
            note,
            created_at: now.clone(),
        };
        repo.add_document(document.clone())?;
        repo.add_event(make_event(
            &client.id,
            &user_id,
            "document_added",
            &client.status,
            &client.status,
            &json!({ "kind": kind }).to_string(),
            &now,
        ))?;
        Ok(document)
    })
    .await?;
    Ok(HttpResponse::Created().json(document))
}

#[post("/clients/{id}/submit")]
pub async fn submit_client(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
) -> Result<HttpResponse, AppError> {
    move_owned(
        state,
        user,
        path.into_inner().id,
        credential::SUBMITTED,
        "submitted",
    )
    .await
}

#[post("/clients/{id}/reopen")]
pub async fn reopen_client(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
) -> Result<HttpResponse, AppError> {
    move_owned(
        state,
        user,
        path.into_inner().id,
        credential::DRAFT,
        "reopened",
    )
    .await
}

#[post("/clients/{id}/review")]
pub async fn review_client(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
    body: web::Json<ReviewBody>,
) -> Result<HttpResponse, AppError> {
    if user.role != "admin" {
        return Err(AppError::Forbidden(
            "somente a equipe Wattly revisa credenciamentos".into(),
        ));
    }
    let action = ReviewAction::parse(&body.action)?;
    let notes = body.notes.clone().unwrap_or_default();
    if matches!(action, ReviewAction::Reject) && notes.trim().is_empty() {
        return Err(AppError::field(
            "notes",
            "uma recusa precisa explicar o motivo",
        ));
    }
    let checks = body.checks.clone().unwrap_or_default();
    let repo = state.repo.clone();
    let reviewer_id = user.id.clone();
    let client_id = path.id.clone();
    let view = blocking(move || {
        require_active(repo.as_ref(), &reviewer_id)?;
        let mut client = repo
            .find_client(&client_id)?
            .ok_or_else(|| AppError::NotFound("credenciamento não encontrado".into()))?;
        let target = action.target();
        transition(&client.status, target)?;
        let now = now_iso();
        if matches!(action, ReviewAction::Approve) {
            validate_checks(
                &checks
                    .iter()
                    .map(|item| (item.item.clone(), item.passed))
                    .collect::<Vec<_>>(),
            )?;
            let records = checks
                .iter()
                .map(|item| CheckRecord {
                    id: Uuid::new_v4().to_string(),
                    client_id: client.id.clone(),
                    item: item.item.clone(),
                    passed: item.passed,
                    note: item.note.clone(),
                    reviewer_id: reviewer_id.clone(),
                    created_at: now.clone(),
                })
                .collect();
            repo.add_checks(records)?;
        }
        let from = client.status.clone();
        client.status = target.to_string();
        client.reviewer_id = Some(reviewer_id.clone());
        client.reviewed_at = Some(now.clone());
        client.updated_at = now.clone();
        client.decision_note = (!notes.trim().is_empty()).then(|| notes.trim().to_string());
        if matches!(action, ReviewAction::Reject) {
            client.reject_reason = Some(notes.trim().to_string());
        }
        let payload = json!({
            "action": body_action(action),
            "notes": notes.trim()
        })
        .to_string();
        let view = client.to_view(true);
        repo.update_client(client.clone())?;
        if let Some(status) = location_status_for(target) {
            repo.set_locations_status(&client.id, status, &now)?;
        }
        repo.add_event(make_event(
            &client.id,
            &reviewer_id,
            action.as_str(),
            &from,
            target,
            &payload,
            &now,
        ))?;
        Ok(view)
    })
    .await?;
    Ok(HttpResponse::Ok().json(view))
}

#[post("/clients/{id}/locations")]
pub async fn create_location(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
    body: web::Json<LocationBody>,
) -> Result<HttpResponse, AppError> {
    let windows: Vec<WindowInput> = body
        .windows
        .iter()
        .map(|window| WindowInput {
            weekday: window.weekday,
            start: window.start.clone(),
            end: window.end.clone(),
        })
        .collect();
    let (draft, window_drafts) = validate_location(LocationInput {
        label: &body.label,
        description: body.description.as_deref().unwrap_or(""),
        address_line: &body.address_line,
        address_number: &body.address_number,
        complement: body.complement.as_deref(),
        district: &body.district,
        city: &body.city,
        state: &body.state,
        postal_code: &body.postal_code,
        country: body.country.as_deref(),
        latitude: body.latitude,
        longitude: body.longitude,
        connector: &body.connector,
        power_kw: body.power_kw,
        voltage: body.voltage,
        phases: body.phases,
        price_per_minute_cents: body.price_per_minute_cents,
        price_per_hour_cents: body.price_per_hour_cents,
        install_type: &body.install_type,
        has_roof: body.has_roof,
        well_lit: body.well_lit,
        access_note: body.access_note.as_deref(),
        spot_code: body.spot_code.as_deref(),
        windows: &windows,
    })?;
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let client_id = path.id.clone();
    let view = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let client = load_owned(&*repo, &client_id, &user_id)?;
        if !client.is_editable() {
            return Err(AppError::Conflict(
                "o ponto só pode ser cadastrado com o credenciamento aberto".into(),
            ));
        }
        let now = now_iso();
        let location_id = Uuid::new_v4().to_string();
        let location = LocationRecord {
            id: location_id.clone(),
            client_id: client.id.clone(),
            label: draft.label,
            description: draft.description,
            address_line: draft.address_line,
            address_number: draft.address_number,
            complement: draft.complement,
            district: draft.district,
            city: draft.city,
            state: draft.state,
            postal_code: draft.postal_code,
            country: draft.country,
            latitude: draft.latitude,
            longitude: draft.longitude,
            public_latitude: 0.0,
            public_longitude: 0.0,
            connector: draft.connector,
            power_kw: draft.power_kw,
            voltage: draft.voltage,
            phases: draft.phases,
            price_per_minute_cents: draft.price_per_minute_cents,
            price_per_hour_cents: draft.price_per_hour_cents,
            install_type: draft.install_type,
            has_roof: draft.has_roof,
            well_lit: draft.well_lit,
            access_note: draft.access_note,
            spot_code: draft.spot_code,
            status: "pending".into(),
            created_at: now.clone(),
            updated_at: now.clone(),
        }
        .with_public_coordinates();
        let windows = window_drafts
            .into_iter()
            .map(|window| WindowRecord {
                id: Uuid::new_v4().to_string(),
                location_id: location_id.clone(),
                weekday: window.weekday,
                start_min: window.start_min,
                end_min: window.end_min,
            })
            .collect::<Vec<_>>();
        let view = location.to_view(&windows, true);
        repo.create_location(location, windows)?;
        repo.add_event(make_event(
            &client.id,
            &user_id,
            "location_added",
            &client.status,
            &client.status,
            &json!({ "location_id": location_id }).to_string(),
            &now,
        ))?;
        Ok(view)
    })
    .await?;
    Ok(HttpResponse::Created().json(view))
}

#[get("/clients/{id}/locations")]
pub async fn list_client_locations(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<ClientPath>,
) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let role = user.role.clone();
    let client_id = path.id.clone();
    let locations = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let client = load_visible(&*repo, &client_id, &user_id, &role)?;
        Ok(repo
            .list_locations_by_client(&client.id)?
            .into_iter()
            .map(|(location, windows)| location.to_view(&windows, true))
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(HttpResponse::Ok().json(json!({ "locations": locations })))
}

async fn move_owned(
    state: web::Data<AppState>,
    user: AuthUser,
    client_id: String,
    target: &'static str,
    event_type: &'static str,
) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let user_id = user.id.clone();
    let view = blocking(move || {
        require_active(repo.as_ref(), &user_id)?;
        let mut client = load_owned(&*repo, &client_id, &user_id)?;
        transition(&client.status, target)?;
        if target == credential::SUBMITTED {
            let locations = repo.list_locations_by_client(&client.id)?;
            let kinds = repo
                .list_documents(&client.id)?
                .into_iter()
                .map(|document| document.kind)
                .collect::<Vec<_>>();
            validate_ready(locations.len(), &kinds)?;
        }
        let now = now_iso();
        let from = client.status.clone();
        client.status = target.to_string();
        client.updated_at = now.clone();
        if target == credential::SUBMITTED {
            client.submitted_at = Some(now.clone());
        }
        if target == credential::DRAFT {
            client.reject_reason = None;
        }
        let view = client.to_view(true);
        repo.update_client(client.clone())?;
        repo.add_event(make_event(
            &client.id, &user_id, event_type, &from, target, "{}", &now,
        ))?;
        Ok(view)
    })
    .await?;
    Ok(HttpResponse::Ok().json(view))
}

fn client_from_draft(draft: NewClient, owner_id: String, now: String) -> ClientRecord {
    ClientRecord {
        id: Uuid::new_v4().to_string(),
        owner_id,
        trade_name: draft.trade_name,
        legal_name: draft.legal_name,
        person_type: draft.person_type,
        tax_id: draft.tax_id,
        pix_key_type: draft.pix_key_type,
        pix_key: draft.pix_key,
        bio: draft.bio,
        status: credential::DRAFT.into(),
        reject_reason: None,
        decision_note: None,
        submitted_at: None,
        reviewed_at: None,
        reviewer_id: None,
        created_at: now.clone(),
        updated_at: now,
    }
}

fn load_owned(
    repo: &dyn crate::repo::Repository,
    client_id: &str,
    user_id: &str,
) -> Result<ClientRecord, AppError> {
    let client = repo
        .find_client(client_id)?
        .ok_or_else(|| AppError::NotFound("credenciamento não encontrado".into()))?;
    if client.owner_id != user_id {
        return Err(AppError::NotFound("credenciamento não encontrado".into()));
    }
    Ok(client)
}

fn load_visible(
    repo: &dyn crate::repo::Repository,
    client_id: &str,
    user_id: &str,
    role: &str,
) -> Result<ClientRecord, AppError> {
    let client = repo
        .find_client(client_id)?
        .ok_or_else(|| AppError::NotFound("credenciamento não encontrado".into()))?;
    if role == "admin" || client.owner_id == user_id {
        Ok(client)
    } else {
        Err(AppError::NotFound("credenciamento não encontrado".into()))
    }
}

fn make_event(
    client_id: &str,
    actor_id: &str,
    event_type: &str,
    from_status: &str,
    to_status: &str,
    payload: &str,
    at: &str,
) -> EventRecord {
    let payload = if payload.chars().count() > 1800 {
        payload.chars().take(1800).collect()
    } else {
        payload.to_string()
    };
    EventRecord {
        id: Uuid::new_v4().to_string(),
        client_id: client_id.to_string(),
        actor_id: actor_id.to_string(),
        event_type: event_type.to_string(),
        from_status: from_status.to_string(),
        to_status: to_status.to_string(),
        payload,
        created_at: at.to_string(),
    }
}

fn body_action(action: ReviewAction) -> &'static str {
    match action {
        ReviewAction::Start => "start",
        ReviewAction::Approve => "approve",
        ReviewAction::Reject => "reject",
        ReviewAction::Suspend => "suspend",
        ReviewAction::Reactivate => "reactivate",
        ReviewAction::Revoke => "revoke",
    }
}
