//! Daedyx Aufgaben-Kern (lokal, sync-fähig). Kein Tauri, keine eigene Datenbank: alles nimmt `&Connection`.
//! Schema und Brücke: `migrate`, Stände in `schema`.
pub mod due_time;
pub mod edit_alarms;
pub mod edit_done;
pub mod edit_lists;
pub mod edit_sections;
pub mod edit_tasks;
pub mod errors;
pub mod hlc;
pub mod ids;
mod import_check;
pub mod merge;
pub mod meta;
pub mod migrate;
pub mod outbox;
#[cfg(test)]
mod prop_tests;
pub mod quick;
#[cfg(test)]
mod quick_tests;
pub mod quick_when;
pub mod reminders;
pub mod repeat;
pub mod schema;
pub mod smart;
pub mod stamp;
pub mod store;
#[cfg(test)]
pub mod test_db;
pub mod tree;
pub mod types;
pub mod validate;
pub mod zone;

pub use errors::TaskError;
pub use migrate::{migrate, MigrateError, SCHEMA_VERSION};
