//! OurFault desktop application.
//!
//! Layers (dependencies point downwards only):
//!
//! ```text
//! commands  – Tauri IPC surface, input/output translation
//! services  – use cases (numbering, creation, distribution, administration)
//! domain    – pure types and business rules
//! adapters  – Excel, SharePoint (mock), distribution (mock), local storage
//! ```
//! `state.rs` wires the concrete adapters together.

mod adapters;
mod commands;
mod domain;
mod error;
mod seed;
mod services;
mod state;

use std::path::PathBuf;

use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime};
use url::Url;

use state::{AppState, Backend, CurrentUser};

/// Internal diagnostics. Never shown to users.
pub(crate) fn log_internal(context: &str, error: &dyn std::fmt::Display) {
    eprintln!("[ourfault] {context}: {error}");
}

pub fn run() {
    tauri::Builder::default()
        .plugin(navigation_guard())
        .setup(|app| {
            let backend = data_dir(app.handle()).and_then(|dir| {
                Backend::open(&dir).map_err(|e| {
                    log_internal("opening local data", &e);
                })
            });
            app.manage(AppState::new(backend.ok(), CurrentUser::from_environment()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_session,
            commands::list_recent_investigations,
            commands::find_investigation,
            commands::parse_pasted_rows,
            commands::list_active_systems,
            commands::peek_next_investigation_number,
            commands::preview_investigation,
            commands::create_investigation,
            commands::compose_distribution,
            commands::distribute_investigation,
            commands::admin_list_systems,
            commands::admin_save_system,
            commands::admin_set_system_active,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OurFault");
}

/// Per-user application data directory (`%APPDATA%\com.ourfault.desktop` on
/// Windows). `OURFAULT_DATA_DIR` overrides it, e.g. for a clean demo.
fn data_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, ()> {
    if let Some(dir) = std::env::var_os("OURFAULT_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    app.path().app_data_dir().map_err(|e| log_internal("resolving data directory", &e))
}

/// Keeps the webview on the bundled frontend: any navigation to another
/// origin (for example through a pasted link) is refused.
fn navigation_guard<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-guard").on_navigation(|_, url| is_app_url(url)).build()
}

fn is_app_url(url: &Url) -> bool {
    match (url.scheme(), url.host_str()) {
        ("tauri", Some("localhost")) => true,
        ("http" | "https", Some("tauri.localhost")) => true,
        ("http", Some("localhost")) => cfg!(debug_assertions) && url.port() == Some(1420),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_is_limited_to_the_app_origin() {
        let allowed = |s: &str| is_app_url(&Url::parse(s).unwrap());
        assert!(allowed("tauri://localhost/index.html"));
        assert!(allowed("http://tauri.localhost/"));
        assert!(!allowed("https://checks.example.com/runs/1"));
        assert!(!allowed("http://localhost:8080/"));
        assert!(!allowed("file:///C:/Windows/win.ini"));
    }
}
