use std::collections::HashSet;
use std::sync::Mutex;

use crate::domain::credential::APPROVED;
use crate::domain::models::{
    CheckRecord, ClientRecord, DocumentRecord, EventRecord, LocationRecord, UserRecord,
    WindowRecord,
};
use crate::error::AppError;

use super::Repository;

const ACTIVE_LOCATION_STATUS: &str = "active";

#[derive(Default)]
struct Db {
    users: Vec<UserRecord>,
    clients: Vec<ClientRecord>,
    locations: Vec<LocationRecord>,
    windows: Vec<WindowRecord>,
    documents: Vec<DocumentRecord>,
    checks: Vec<CheckRecord>,
    events: Vec<EventRecord>,
}

pub struct MemoryRepo {
    inner: Mutex<Db>,
}

impl MemoryRepo {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Db::default()),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Db>, AppError> {
        self.inner.lock().map_err(|_| AppError::Internal)
    }
}

impl Default for MemoryRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl Repository for MemoryRepo {
    fn ping(&self) -> Result<String, AppError> {
        Ok("memory".into())
    }

    fn create_user(&self, user: UserRecord) -> Result<(), AppError> {
        let mut db = self.lock()?;
        if db.users.iter().any(|item| item.email == user.email) {
            return Err(AppError::Conflict("e-mail já cadastrado".into()));
        }
        db.users.push(user);
        Ok(())
    }

    fn find_user_by_email(&self, email: &str) -> Result<Option<UserRecord>, AppError> {
        let db = self.lock()?;
        Ok(db.users.iter().find(|user| user.email == email).cloned())
    }

    fn find_user_by_id(&self, id: &str) -> Result<Option<UserRecord>, AppError> {
        let db = self.lock()?;
        Ok(db.users.iter().find(|user| user.id == id).cloned())
    }

    fn set_last_login(&self, id: &str, at: &str) -> Result<(), AppError> {
        let mut db = self.lock()?;
        let user = db
            .users
            .iter_mut()
            .find(|user| user.id == id)
            .ok_or_else(|| AppError::NotFound("usuário não encontrado".into()))?;
        user.last_login_at = Some(at.to_string());
        user.updated_at = at.to_string();
        Ok(())
    }

    fn set_user_status(&self, id: &str, status: &str, at: &str) -> Result<(), AppError> {
        let mut db = self.lock()?;
        let user = db
            .users
            .iter_mut()
            .find(|user| user.id == id)
            .ok_or_else(|| AppError::NotFound("usuário não encontrado".into()))?;
        user.status = status.to_string();
        user.updated_at = at.to_string();
        Ok(())
    }

    fn create_client(&self, client: ClientRecord) -> Result<(), AppError> {
        let mut db = self.lock()?;
        if !db.users.iter().any(|user| user.id == client.owner_id) {
            return Err(AppError::NotFound("usuário não encontrado".into()));
        }
        db.clients.push(client);
        Ok(())
    }

    fn update_client(&self, client: ClientRecord) -> Result<(), AppError> {
        let mut db = self.lock()?;
        let slot = db
            .clients
            .iter_mut()
            .find(|item| item.id == client.id)
            .ok_or_else(|| AppError::NotFound("credenciamento não encontrado".into()))?;
        *slot = client;
        Ok(())
    }

    fn find_client(&self, id: &str) -> Result<Option<ClientRecord>, AppError> {
        let db = self.lock()?;
        Ok(db.clients.iter().find(|client| client.id == id).cloned())
    }

    fn list_clients_by_owner(&self, owner_id: &str) -> Result<Vec<ClientRecord>, AppError> {
        let db = self.lock()?;
        let mut clients: Vec<_> = db
            .clients
            .iter()
            .filter(|client| client.owner_id == owner_id)
            .cloned()
            .collect();
        clients.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(clients)
    }

    fn list_clients(&self) -> Result<Vec<ClientRecord>, AppError> {
        let db = self.lock()?;
        let mut clients = db.clients.clone();
        clients.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(clients)
    }

    fn create_location(
        &self,
        location: LocationRecord,
        windows: Vec<WindowRecord>,
    ) -> Result<(), AppError> {
        let mut db = self.lock()?;
        if !db
            .clients
            .iter()
            .any(|client| client.id == location.client_id)
        {
            return Err(AppError::NotFound("credenciamento não encontrado".into()));
        }
        db.locations.push(location);
        db.windows.extend(windows);
        Ok(())
    }

    fn list_locations_by_client(
        &self,
        client_id: &str,
    ) -> Result<Vec<(LocationRecord, Vec<WindowRecord>)>, AppError> {
        let db = self.lock()?;
        Ok(db
            .locations
            .iter()
            .filter(|location| location.client_id == client_id)
            .cloned()
            .map(|location| {
                let windows = windows_of(&db, &location.id);
                (location, windows)
            })
            .collect())
    }

    fn find_location(
        &self,
        id: &str,
    ) -> Result<Option<(LocationRecord, Vec<WindowRecord>)>, AppError> {
        let db = self.lock()?;
        Ok(db
            .locations
            .iter()
            .find(|location| location.id == id)
            .cloned()
            .map(|location| {
                let windows = windows_of(&db, &location.id);
                (location, windows)
            }))
    }

    fn list_public_locations(&self) -> Result<Vec<(LocationRecord, Vec<WindowRecord>)>, AppError> {
        let db = self.lock()?;
        let approved: HashSet<_> = db
            .clients
            .iter()
            .filter(|client| client.status == APPROVED)
            .map(|client| client.id.clone())
            .collect();
        Ok(db
            .locations
            .iter()
            .filter(|location| {
                location.status == ACTIVE_LOCATION_STATUS && approved.contains(&location.client_id)
            })
            .cloned()
            .map(|location| {
                let windows = windows_of(&db, &location.id);
                (location, windows)
            })
            .collect())
    }

    fn set_locations_status(
        &self,
        client_id: &str,
        status: &str,
        at: &str,
    ) -> Result<(), AppError> {
        let mut db = self.lock()?;
        for location in &mut db.locations {
            if location.client_id == client_id {
                location.status = status.to_string();
                location.updated_at = at.to_string();
            }
        }
        Ok(())
    }

    fn add_document(&self, document: DocumentRecord) -> Result<(), AppError> {
        let mut db = self.lock()?;
        if !db
            .clients
            .iter()
            .any(|client| client.id == document.client_id)
        {
            return Err(AppError::NotFound("credenciamento não encontrado".into()));
        }
        db.documents.push(document);
        Ok(())
    }

    fn list_documents(&self, client_id: &str) -> Result<Vec<DocumentRecord>, AppError> {
        let db = self.lock()?;
        Ok(db
            .documents
            .iter()
            .filter(|document| document.client_id == client_id)
            .cloned()
            .collect())
    }

    fn add_checks(&self, checks: Vec<CheckRecord>) -> Result<(), AppError> {
        let mut db = self.lock()?;
        db.checks.extend(checks);
        Ok(())
    }

    fn list_checks(&self, client_id: &str) -> Result<Vec<CheckRecord>, AppError> {
        let db = self.lock()?;
        Ok(db
            .checks
            .iter()
            .filter(|check| check.client_id == client_id)
            .cloned()
            .collect())
    }

    fn add_event(&self, event: EventRecord) -> Result<(), AppError> {
        let mut db = self.lock()?;
        db.events.push(event);
        Ok(())
    }

    fn list_events(&self, client_id: &str) -> Result<Vec<EventRecord>, AppError> {
        let db = self.lock()?;
        let mut events: Vec<_> = db
            .events
            .iter()
            .filter(|event| event.client_id == client_id)
            .cloned()
            .collect();
        events.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(events)
    }
}

fn windows_of(db: &Db, location_id: &str) -> Vec<WindowRecord> {
    db.windows
        .iter()
        .filter(|window| window.location_id == location_id)
        .cloned()
        .collect()
}
