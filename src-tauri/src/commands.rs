//! Tauri commands: the complete API available to the webview.
//!
//! Each command is thin: it translates IPC input, calls a service and returns
//! a serialisable result or an [`AppError`]. Every command must also be listed
//! in `build.rs` and granted in `capabilities/main-window.json`.

use tauri::State;

use crate::domain::distribution::DistributionMessage;
use crate::domain::investigation::{Investigation, InvestigationDraft, InvestigationSummary, StoredInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::log_rows::{self, PastedRows};
use crate::domain::system::{System, SystemInput};
use crate::error::AppError;
use crate::services::distribution::{DistributionService, SentDistribution};
use crate::services::investigations::InvestigationService;
use crate::services::systems::SystemService;
use crate::services::Now;
use crate::state::{AppState, CurrentUser};

const RECENT_LIMIT: usize = 8;

type CommandResult<T> = Result<T, AppError>;

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

// ---- Pasted rows -----------------------------------------------------------

/// Parses rows the operator copied in Excel and pasted into the app. The text
/// comes from the paste event only; the app has no clipboard permission.
#[tauri::command]
pub async fn parse_pasted_rows(text: String) -> CommandResult<PastedRows> {
    Ok(log_rows::parse_pasted_rows(&text)?)
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
    investigations(&state)?.preview(&draft, &Now::local())
}

#[tauri::command]
pub async fn create_investigation(
    state: State<'_, AppState>,
    draft: InvestigationDraft,
) -> CommandResult<StoredInvestigation> {
    investigations(&state)?.create(&draft, &Now::local(), &state.user.display_name)
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
