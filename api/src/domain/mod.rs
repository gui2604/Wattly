pub mod credential;
pub mod geo;
pub mod models;
pub mod privacy;
pub mod validation;

pub const TERMS_VERSION: &str = "2026-11";

pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
