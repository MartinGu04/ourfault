//! OurFault desktop application.
//!
//! Layers (dependencies point downwards only):
//!
//! ```text
//! commands  – Tauri IPC surface, input/output translation
//! services  – use cases (numbering, creation, distribution, administration)
//! domain    – pure types and business rules
//! render    – document view, HTML and PDF output (pure)
//! adapters  – configuration and draft storage, publishers (mock SharePoint,
//!             shared folder), distribution (mock), exported files
//! ```
//! `state.rs` wires the concrete adapters together and holds the work mode.

mod adapters;
mod commands;
mod domain;
mod error;
mod render;
mod seed;
mod services;
mod state;

use std::path::PathBuf;

use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime};
use url::Url;

use state::{AppState, Backend, BackendOptions, DemoAccessPolicy, PublisherChoice};

/// Internal diagnostics. Never shown to users.
pub(crate) fn log_internal(context: &str, error: &dyn std::fmt::Display) {
    eprintln!("[ourfault] {context}: {error}");
}

pub fn run() {
    tauri::Builder::default()
        .plugin(navigation_guard())
        .setup(|app| {
            let backend = backend_options(app.handle()).and_then(|options| {
                Backend::open(&options).map_err(|e| {
                    log_internal("opening local data", &e);
                })
            });
            app.manage(AppState::new(backend.ok(), Box::new(DemoAccessPolicy::from_environment()), operator_name()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_session,
            commands::enter_work_mode,
            commands::leave_work_mode,
            commands::get_workspace,
            commands::list_drafts,
            commands::get_draft,
            commands::create_draft,
            commands::save_draft,
            commands::delete_draft,
            commands::parse_pasted_rows,
            commands::review_draft,
            commands::complete_draft,
            commands::list_recent_investigations,
            commands::search_investigations,
            commands::get_investigation,
            commands::compose_distribution,
            commands::distribute_investigation,
            commands::export_investigation_pdf,
            commands::export_draft_pdf,
            commands::admin_get_configuration,
            commands::admin_save_system,
            commands::admin_set_system_active,
            commands::admin_save_station,
            commands::admin_set_station_active,
            commands::admin_save_section,
            commands::admin_set_section_active,
            commands::admin_move_section,
            commands::admin_save_mail_template,
            commands::admin_save_publication,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OurFault");
}

/// Where local data, exports and publications go.
///
/// * data: per-user application data directory (`%APPDATA%\com.ourfault.desktop`
///   on Windows); `OURFAULT_DATA_DIR` overrides it, e.g. for a clean demo.
/// * exports: `Downloads\OurFault`; `OURFAULT_EXPORT_DIR` overrides it.
/// * publication: the SharePoint simulation, or with `OURFAULT_PUBLISHER=folder`
///   the existing folder `OURFAULT_PUBLISH_DIR`.
fn backend_options<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<BackendOptions, ()> {
    let path = app.path();
    let data_dir = match std::env::var_os("OURFAULT_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => path.app_data_dir().map_err(|e| log_internal("resolving data directory", &e))?,
    };
    let export_dir = match std::env::var_os("OURFAULT_EXPORT_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => path.download_dir().map(|dir| dir.join("OurFault")).unwrap_or_else(|_| data_dir.join("exports")),
    };
    let publisher = match (std::env::var("OURFAULT_PUBLISHER").as_deref(), std::env::var_os("OURFAULT_PUBLISH_DIR")) {
        (Ok("folder"), Some(dir)) => PublisherChoice::SharedFolder(PathBuf::from(dir)),
        _ => PublisherChoice::MockSharePoint,
    };
    Ok(BackendOptions { data_dir, export_dir, publisher })
}

/// The Windows user name, recorded as author of drafts and status changes.
/// Not an authentication mechanism.
fn operator_name() -> String {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "OurFault".into())
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
