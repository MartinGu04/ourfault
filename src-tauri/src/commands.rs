//! Tauri commands: the complete API available to the webview.
//!
//! Each command is thin: it translates IPC input, calls a service and returns
//! a serialisable result or an [`AppError`]. Every command must also be listed
//! in `build.rs` and granted in `capabilities/main-window.json`.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::adapters::excel::{self, ImportError};
use crate::domain::distribution::DistributionMessage;
use crate::domain::investigation::{Investigation, InvestigationDraft, InvestigationSummary, StoredInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::operations_log::OperationsLog;
use crate::domain::system::{System, SystemInput};
use crate::error::AppError;
use crate::seed;
use crate::services::distribution::{DistributionService, SentDistribution};
use crate::services::investigations::InvestigationService;
use crate::services::systems::SystemService;
use crate::services::Now;
use crate::state::{AppState, CurrentUser};

const RECENT_LIMIT: usize = 8;

type CommandResult<T> = Result<T, AppError>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedLog {
    import_id: u64,
    #[serde(flatten)]
    log: OperationsLog,
}

fn investigations(state: &AppState) -> CommandResult<InvestigationService<'_>> {
    let backend = state.backend()?;
    Ok(InvestigationService::new(backend.sharepoint.as_ref(), backend.systems.as_ref()))
}

fn systems(state: &AppState) -> CommandResult<SystemService<'_>> {
    Ok(SystemService::new(state.backend()?.systems.as_ref()))
}

fn distribution(state: &AppState) -> CommandResult<DistributionService<'_>> {
    let backend = state.backend()?;
    Ok(DistributionService::new(backend.sharepoint.as_ref(), backend.systems.as_ref(), backend.distribution.as_ref()))
}

fn parse_number(number: &str) -> CommandResult<InvestigationNumber> {
    number.parse().map_err(|_| AppError::field("number", "invalid_number"))
}

// ---- Session & home -------------------------------------------------------

#[tauri::command]
pub async fn get_session(state: State<'_, AppState>) -> CommandResult<CurrentUser> {
    state.backend()?;
    Ok(state.user.clone())
}

#[tauri::command]
pub async fn list_recent_investigations(state: State<'_, AppState>) -> CommandResult<Vec<InvestigationSummary>> {
    investigations(&state)?.recent(RECENT_LIMIT)
}

#[tauri::command]
pub async fn find_investigation(state: State<'_, AppState>, query: String) -> CommandResult<StoredInvestigation> {
    investigations(&state)?.find(&query, &Now::local())
}

// ---- Import -----------------------------------------------------------------

/// Shows the native file picker and imports the chosen workbook. The path is
/// chosen by the user in the OS dialog; the webview never supplies a path.
#[tauri::command]
pub async fn import_workbook(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Option<ImportedLog>> {
    let picked =
        app.dialog().file().set_title("בחירת יומן מבצעים").add_filter("Excel (xlsx)", &["xlsx"]).blocking_pick_file();
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| AppError::internal("file dialog", &e))?;
    let bytes = read_workbook_file(&path)?;
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    import_bytes(&state, &bytes, &file_name).map(Some)
}

/// Imports the fictional workbook bundled with the application.
#[tauri::command]
pub async fn import_demo_workbook(state: State<'_, AppState>) -> CommandResult<ImportedLog> {
    import_bytes(&state, seed::DEMO_WORKBOOK, seed::DEMO_WORKBOOK_NAME)
}

fn read_workbook_file(path: &Path) -> CommandResult<Vec<u8>> {
    let is_xlsx = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("xlsx"));
    if !is_xlsx {
        return Err(AppError::Import { code: "unsupported_file_type", missing_columns: Vec::new() });
    }
    let file = File::open(path).map_err(|e| AppError::Import {
        code: if e.kind() == std::io::ErrorKind::PermissionDenied { "file_locked" } else { "file_unreadable" },
        missing_columns: Vec::new(),
    })?;
    let mut bytes = Vec::new();
    file.take(excel::MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::Import { code: "file_unreadable", missing_columns: Vec::new() })?;
    if bytes.len() as u64 > excel::MAX_FILE_BYTES {
        return Err(ImportError::TooLarge.into());
    }
    Ok(bytes)
}

fn import_bytes(state: &AppState, bytes: &[u8], file_name: &str) -> CommandResult<ImportedLog> {
    let highlight = state.backend()?.settings.highlight_color();
    let log = excel::read_operations_log(bytes, file_name, &highlight)?;
    let import_id = state.set_import(log.clone())?;
    Ok(ImportedLog { import_id, log })
}

// ---- Investigation wizard ---------------------------------------------------

#[tauri::command]
pub async fn list_active_systems(state: State<'_, AppState>) -> CommandResult<Vec<System>> {
    systems(&state)?.active()
}

#[tauri::command]
pub async fn peek_next_investigation_number(state: State<'_, AppState>) -> CommandResult<InvestigationNumber> {
    investigations(&state)?.next_number(&Now::local())
}

#[tauri::command]
pub async fn preview_investigation(
    state: State<'_, AppState>,
    draft: InvestigationDraft,
) -> CommandResult<Investigation> {
    let service = investigations(&state)?;
    state.with_import(draft.import_id, |log| service.preview(&draft, log, &Now::local()))
}

#[tauri::command]
pub async fn create_investigation(
    state: State<'_, AppState>,
    draft: InvestigationDraft,
) -> CommandResult<StoredInvestigation> {
    let service = investigations(&state)?;
    let user = state.user.display_name.clone();
    state.with_import(draft.import_id, |log| service.create(&draft, log, &Now::local(), &user))
}

// ---- Distribution -----------------------------------------------------------

#[tauri::command]
pub async fn compose_distribution(state: State<'_, AppState>, number: String) -> CommandResult<DistributionMessage> {
    distribution(&state)?.compose(parse_number(&number)?)
}

#[tauri::command]
pub async fn distribute_investigation(state: State<'_, AppState>, number: String) -> CommandResult<SentDistribution> {
    distribution(&state)?.distribute(parse_number(&number)?, &Now::local())
}

// ---- Administration (admin only) --------------------------------------------

#[tauri::command]
pub async fn admin_list_systems(state: State<'_, AppState>) -> CommandResult<Vec<System>> {
    state.require_admin()?;
    systems(&state)?.list()
}

#[tauri::command]
pub async fn admin_save_system(state: State<'_, AppState>, input: SystemInput) -> CommandResult<System> {
    state.require_admin()?;
    systems(&state)?.save(&input)
}

#[tauri::command]
pub async fn admin_set_system_active(state: State<'_, AppState>, id: String, active: bool) -> CommandResult<System> {
    state.require_admin()?;
    systems(&state)?.set_active(&id, active)
}
