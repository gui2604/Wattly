mod memory;
pub mod oracle;

pub use memory::MemoryRepo;

use crate::domain::models::{
    CheckRecord, ClientRecord, DocumentRecord, EventRecord, LocationRecord, UserRecord,
    WindowRecord,
};
use crate::error::AppError;

pub trait Repository: Send + Sync {
    fn ping(&self) -> Result<String, AppError>;
    fn create_user(&self, user: UserRecord) -> Result<(), AppError>;
    fn find_user_by_email(&self, email: &str) -> Result<Option<UserRecord>, AppError>;
    fn find_user_by_id(&self, id: &str) -> Result<Option<UserRecord>, AppError>;
    fn set_last_login(&self, id: &str, at: &str) -> Result<(), AppError>;
    fn set_user_status(&self, id: &str, status: &str, at: &str) -> Result<(), AppError>;

    fn create_client(&self, client: ClientRecord) -> Result<(), AppError>;
    fn update_client(&self, client: ClientRecord) -> Result<(), AppError>;
    fn find_client(&self, id: &str) -> Result<Option<ClientRecord>, AppError>;
    fn list_clients_by_owner(&self, owner_id: &str) -> Result<Vec<ClientRecord>, AppError>;
    fn list_clients(&self) -> Result<Vec<ClientRecord>, AppError>;

    fn create_location(
        &self,
        location: LocationRecord,
        windows: Vec<WindowRecord>,
    ) -> Result<(), AppError>;
    fn list_locations_by_client(
        &self,
        client_id: &str,
    ) -> Result<Vec<(LocationRecord, Vec<WindowRecord>)>, AppError>;
    fn find_location(
        &self,
        id: &str,
    ) -> Result<Option<(LocationRecord, Vec<WindowRecord>)>, AppError>;
    fn list_public_locations(&self) -> Result<Vec<(LocationRecord, Vec<WindowRecord>)>, AppError>;
    fn set_locations_status(&self, client_id: &str, status: &str, at: &str)
        -> Result<(), AppError>;

    fn add_document(&self, document: DocumentRecord) -> Result<(), AppError>;
    fn list_documents(&self, client_id: &str) -> Result<Vec<DocumentRecord>, AppError>;
    fn add_checks(&self, checks: Vec<CheckRecord>) -> Result<(), AppError>;
    fn list_checks(&self, client_id: &str) -> Result<Vec<CheckRecord>, AppError>;
    fn add_event(&self, event: EventRecord) -> Result<(), AppError>;
    fn list_events(&self, client_id: &str) -> Result<Vec<EventRecord>, AppError>;
}
