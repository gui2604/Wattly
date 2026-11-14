use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::domain::credential::APPROVED;
use crate::domain::geo::haversine_km;
use crate::error::AppError;
use crate::state::AppState;

use super::{blocking, require_active, MaybeUser};

#[derive(Debug, Deserialize)]
struct NearbyQuery {
    lat: f64,
    lng: f64,
    radius_km: Option<f64>,
    connector: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct LocationPath {
    id: String,
}

#[get("/locations")]
pub async fn search_locations(
    state: web::Data<AppState>,
    query: web::Query<NearbyQuery>,
) -> Result<HttpResponse, AppError> {
    if !query.lat.is_finite() || !(-90.0..=90.0).contains(&query.lat) {
        return Err(AppError::field("lat", "latitude inválida"));
    }
    if !query.lng.is_finite() || !(-180.0..=180.0).contains(&query.lng) {
        return Err(AppError::field("lng", "longitude inválida"));
    }
    let radius = query.radius_km.unwrap_or(5.0);
    if !radius.is_finite() || !(0.1..=50.0).contains(&radius) {
        return Err(AppError::field(
            "radius_km",
            "use um raio entre 0,1 e 50 km",
        ));
    }
    if let Some(connector) = &query.connector {
        if !matches!(
            connector.as_str(),
            "home_outlet_220v" | "wallbox_type2" | "wallbox_ccs2"
        ) {
            return Err(AppError::field(
                "connector",
                "use home_outlet_220v, wallbox_type2 ou wallbox_ccs2",
            ));
        }
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let lat = query.lat;
    let lng = query.lng;
    let connector = query.connector.clone();
    let repo = state.repo.clone();
    let locations = blocking(move || {
        let rows = repo.list_public_locations()?;
        let locations = rows
            .into_iter()
            .filter(|(location, _)| {
                let distance = haversine_km(
                    lat,
                    lng,
                    location.public_latitude,
                    location.public_longitude,
                );
                let connector_ok = connector
                    .as_ref()
                    .is_none_or(|expected| expected == &location.connector);
                distance <= radius && connector_ok
            })
            .take(limit)
            .map(|(location, windows)| location.to_view(&windows, false))
            .collect::<Vec<_>>();
        Ok(locations)
    })
    .await?;
    Ok(HttpResponse::Ok().json(json!({ "locations": locations })))
}

#[get("/locations/{id}")]
pub async fn get_location(
    state: web::Data<AppState>,
    viewer: MaybeUser,
    path: web::Path<LocationPath>,
) -> Result<HttpResponse, AppError> {
    let repo = state.repo.clone();
    let location_id = path.id.clone();
    let viewer = viewer.0;
    let view = blocking(move || {
        if let Some(user) = &viewer {
            require_active(repo.as_ref(), &user.id)?;
        }
        let (location, windows) = repo
            .find_location(&location_id)?
            .ok_or_else(|| AppError::NotFound("ponto não encontrado".into()))?;
        let client = repo
            .find_client(&location.client_id)?
            .ok_or_else(|| AppError::NotFound("ponto não encontrado".into()))?;
        let privileged = viewer
            .as_ref()
            .is_some_and(|user| user.role == "admin" || user.id == client.owner_id);
        if !privileged && (client.status != APPROVED || location.status != "active") {
            return Err(AppError::NotFound("ponto não encontrado".into()));
        }
        Ok(location.to_view(&windows, privileged))
    })
    .await?;
    Ok(HttpResponse::Ok().json(view))
}
