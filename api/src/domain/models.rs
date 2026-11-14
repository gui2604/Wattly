use serde::Serialize;

use super::credential::{APPROVED, SUSPENDED};
use super::geo::approximate;
use super::privacy::{mask_document, mask_pix};

#[derive(Clone, Debug)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub full_name: String,
    pub phone: String,
    pub document: String,
    pub role: String,
    pub status: String,
    pub terms_version: String,
    pub consent_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublicUser {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub phone: String,
    pub document_masked: String,
    pub role: String,
    pub status: String,
    pub terms_version: String,
    pub consent_at: String,
    pub created_at: String,
}

impl UserRecord {
    pub fn to_public(&self) -> PublicUser {
        PublicUser {
            id: self.id.clone(),
            email: self.email.clone(),
            full_name: self.full_name.clone(),
            phone: self.phone.clone(),
            document_masked: mask_document(&self.document),
            role: self.role.clone(),
            status: self.status.clone(),
            terms_version: self.terms_version.clone(),
            consent_at: self.consent_at.clone(),
            created_at: self.created_at.clone(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
}

#[derive(Clone, Debug)]
pub struct ClientRecord {
    pub id: String,
    pub owner_id: String,
    pub trade_name: String,
    pub legal_name: String,
    pub person_type: String,
    pub tax_id: String,
    pub pix_key_type: String,
    pub pix_key: String,
    pub bio: String,
    pub status: String,
    pub reject_reason: Option<String>,
    pub decision_note: Option<String>,
    pub submitted_at: Option<String>,
    pub reviewed_at: Option<String>,
    pub reviewer_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClientView {
    pub id: String,
    pub owner_id: String,
    pub trade_name: String,
    pub legal_name: String,
    pub person_type: String,
    pub tax_id_masked: String,
    pub tax_id: Option<String>,
    pub pix_key_type: String,
    pub pix_key_masked: String,
    pub pix_key: Option<String>,
    pub bio: String,
    pub status: String,
    pub reject_reason: Option<String>,
    pub decision_note: Option<String>,
    pub submitted_at: Option<String>,
    pub reviewed_at: Option<String>,
    pub reviewer_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ClientRecord {
    pub fn to_view(&self, privileged: bool) -> ClientView {
        ClientView {
            id: self.id.clone(),
            owner_id: self.owner_id.clone(),
            trade_name: self.trade_name.clone(),
            legal_name: self.legal_name.clone(),
            person_type: self.person_type.clone(),
            tax_id_masked: mask_document(&self.tax_id),
            tax_id: privileged.then(|| self.tax_id.clone()),
            pix_key_type: self.pix_key_type.clone(),
            pix_key_masked: mask_pix(&self.pix_key_type, &self.pix_key),
            pix_key: privileged.then(|| self.pix_key.clone()),
            bio: self.bio.clone(),
            status: self.status.clone(),
            reject_reason: self.reject_reason.clone(),
            decision_note: self.decision_note.clone(),
            submitted_at: self.submitted_at.clone(),
            reviewed_at: self.reviewed_at.clone(),
            reviewer_id: self.reviewer_id.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }

    pub fn is_editable(&self) -> bool {
        self.status == "draft" || self.status == "rejected"
    }

    pub fn is_publicly_visible(&self) -> bool {
        self.status == APPROVED
    }

    pub fn is_suspended(&self) -> bool {
        self.status == SUSPENDED
    }
}

#[derive(Clone, Debug)]
pub struct LocationRecord {
    pub id: String,
    pub client_id: String,
    pub label: String,
    pub description: String,
    pub address_line: String,
    pub address_number: String,
    pub complement: Option<String>,
    pub district: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
    pub public_latitude: f64,
    pub public_longitude: f64,
    pub connector: String,
    pub power_kw: f64,
    pub voltage: i32,
    pub phases: i32,
    pub price_per_minute_cents: i64,
    pub price_per_hour_cents: i64,
    pub install_type: String,
    pub has_roof: bool,
    pub well_lit: bool,
    pub access_note: Option<String>,
    pub spot_code: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl LocationRecord {
    pub fn with_public_coordinates(mut self) -> Self {
        self.public_latitude = approximate(self.latitude);
        self.public_longitude = approximate(self.longitude);
        self
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowView {
    pub weekday: i32,
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug)]
pub struct WindowRecord {
    pub id: String,
    pub location_id: String,
    pub weekday: i32,
    pub start_min: i32,
    pub end_min: i32,
}

impl WindowRecord {
    pub fn to_view(&self) -> WindowView {
        WindowView {
            weekday: self.weekday,
            start: minutes_to_clock(self.start_min),
            end: minutes_to_clock(self.end_min),
        }
    }
}

fn minutes_to_clock(minutes: i32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

#[derive(Clone, Debug, Serialize)]
pub struct LocationView {
    pub id: String,
    pub client_id: String,
    pub label: String,
    pub description: String,
    pub district: String,
    pub city: String,
    pub state: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
    pub approximate: bool,
    pub connector: String,
    pub power_kw: f64,
    pub voltage: i32,
    pub phases: i32,
    pub price_per_minute_cents: i64,
    pub price_per_hour_cents: i64,
    pub install_type: String,
    pub has_roof: bool,
    pub well_lit: bool,
    pub status: String,
    pub windows: Vec<WindowView>,
    pub address_line: Option<String>,
    pub address_number: Option<String>,
    pub complement: Option<String>,
    pub postal_code: Option<String>,
    pub access_note: Option<String>,
    pub spot_code: Option<String>,
    pub created_at: String,
}

impl LocationRecord {
    pub fn to_view(&self, windows: &[WindowRecord], privileged: bool) -> LocationView {
        LocationView {
            id: self.id.clone(),
            client_id: self.client_id.clone(),
            label: self.label.clone(),
            description: self.description.clone(),
            district: self.district.clone(),
            city: self.city.clone(),
            state: self.state.clone(),
            country: self.country.clone(),
            latitude: if privileged {
                self.latitude
            } else {
                self.public_latitude
            },
            longitude: if privileged {
                self.longitude
            } else {
                self.public_longitude
            },
            approximate: !privileged,
            connector: self.connector.clone(),
            power_kw: self.power_kw,
            voltage: self.voltage,
            phases: self.phases,
            price_per_minute_cents: self.price_per_minute_cents,
            price_per_hour_cents: self.price_per_hour_cents,
            install_type: self.install_type.clone(),
            has_roof: self.has_roof,
            well_lit: self.well_lit,
            status: self.status.clone(),
            windows: windows.iter().map(WindowRecord::to_view).collect(),
            address_line: privileged.then(|| self.address_line.clone()),
            address_number: privileged.then(|| self.address_number.clone()),
            complement: privileged.then(|| self.complement.clone()).flatten(),
            postal_code: privileged.then(|| self.postal_code.clone()),
            access_note: privileged.then(|| self.access_note.clone()).flatten(),
            spot_code: privileged.then(|| self.spot_code.clone()).flatten(),
            created_at: self.created_at.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DocumentRecord {
    pub id: String,
    pub client_id: String,
    pub kind: String,
    pub storage_ref: String,
    pub status: String,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckRecord {
    pub id: String,
    pub client_id: String,
    pub item: String,
    pub passed: bool,
    pub note: Option<String>,
    pub reviewer_id: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventRecord {
    pub id: String,
    pub client_id: String,
    pub actor_id: String,
    pub event_type: String,
    pub from_status: String,
    pub to_status: String,
    pub payload: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Dossier {
    pub client: ClientView,
    pub documents: Vec<DocumentRecord>,
    pub checks: Vec<CheckRecord>,
    pub events: Vec<EventRecord>,
    pub locations: Vec<LocationView>,
}
