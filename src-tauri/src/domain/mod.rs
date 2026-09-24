//! Pure business types and rules. Nothing in this module performs I/O or
//! depends on Tauri, which keeps it easy to test and to reuse.

pub mod distribution;
pub mod investigation;
pub mod investigation_number;
pub mod log_rows;
pub mod system;
pub mod validation;
