//! Pure business types and rules. Nothing in this module performs I/O or
//! depends on Tauri, which keeps it easy to test and to reuse.

pub mod activity;
pub mod advisories;
pub mod configuration;
pub mod draft;
pub mod investigation;
pub mod investigation_number;
pub mod lifecycle;
pub mod log_rows;
pub mod mail;
pub mod night_window;
pub mod sections;
pub mod station;
pub mod system;
pub mod validation;
