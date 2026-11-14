use std::sync::Arc;

use crate::repo::{MemoryRepo, Repository};

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn Repository>,
    pub jwt_secret: Arc<str>,
    pub token_ttl_hours: i64,
}

impl AppState {
    pub fn memory(secret: &str) -> Self {
        Self {
            repo: Arc::new(MemoryRepo::new()),
            jwt_secret: Arc::from(secret),
            token_ttl_hours: 12,
        }
    }
}
