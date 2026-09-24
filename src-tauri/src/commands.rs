//! Tauri commands: the complete API available to the webview.
//!
//! Each command is thin: it translates IPC input, calls a service and returns
//! a serialisable result or an [`AppError`]. Every command must also be listed
//! in `build.rs` and granted in `capabilities/main-window.json`. Admin
//! commands obtain their service only through [`AppState::admin`].

use tauri::State;

use crate::adapters::ExportedFile;
use crate::domain::configuration::PublicationSettings;
use crate::domain::draft::{Draft, DraftContent, DraftStep, DraftSummary};
use crate::domain::investigation::InvestigationSummary;
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::log_rows::{self, PastedRows};
use crate::domain::mail::{DistributionMessage, MailTemplate};
use crate::domain::sections::{SectionDefinition, SectionInput};
use crate::domain::station::{Station, StationInput};
use crate::domain::system::{System, SystemInput};
use crate::error::AppError;
use crate::services::configuration::{AdminConfiguration, ConfigurationService, Workspace};
use crate::services::distribution::{DistributionService, SentDistribution};
use crate::services::drafts::DraftService;
use crate::services::export::ExportService;
use crate::services::investigations::{Completion, InvestigationDetails, InvestigationService, Review};
use crate::services::Now;
use crate::state::{AppState, Session, WorkMode};

const RECENT_LIMIT: usize = 8;

type CommandResult<T> = Result<T, AppError>;

fn drafts(state: &AppState) -> CommandResult<DraftService<'_>> {
    let backend = state.backend()?;
    Ok(DraftService::new(backend.drafts.as_ref(), backend.configuration.as_ref()))
}

fn investigations(state: &AppState) -> CommandResult<InvestigationService<'_>> {
    let backend = state.backend()?;
    Ok(InvestigationService::new(backend.publisher.as_ref(), backend.configuration.as_ref(), backend.drafts.as_ref()))
}

fn distribution(state: &AppState) -> CommandResult<DistributionService<'_>> {
    let backend = state.backend()?;
    Ok(DistributionService::new(
        backend.publisher.as_ref(),
        backend.configuration.as_ref(),
        backend.distribution.as_ref(),
    ))
}

fn export(state: &AppState) -> CommandResult<ExportService<'_>> {
    let backend = state.backend()?;
    Ok(ExportService::new(
        backend.publisher.as_ref(),
        backend.configuration.as_ref(),
        backend.drafts.as_ref(),
        backend.exports.as_ref(),
    ))
}

fn parse_number(number: &str) -> CommandResult<InvestigationNumber> {
    number.parse().map_err(|_| AppError::field("number", "invalid_number"))
}

// ---- Session and work mode --------------------------------------------------

#[tauri::command]
pub async fn get_session(state: State<'_, AppState>) -> CommandResult<Session> {
    state.backend()?;
    Ok(state.session())
}

/// Chooses a work mode on the entry screen. Not authentication: the access
/// policy decides which modes are available.
#[tauri::command]
pub async fn enter_work_mode(state: State<'_, AppState>, mode: WorkMode) -> CommandResult<Session> {
    state.enter(mode)
}

#[tauri::command]
pub async fn leave_work_mode(state: State<'_, AppState>) -> CommandResult<Session> {
    Ok(state.leave())
}

#[tauri::command]
pub async fn get_workspace(state: State<'_, AppState>) -> CommandResult<Workspace> {
    ConfigurationService::new(state.backend()?.configuration.as_ref()).workspace()
}

// ---- Drafts -----------------------------------------------------------------

#[tauri::command]
pub async fn list_drafts(state: State<'_, AppState>) -> CommandResult<Vec<DraftSummary>> {
    drafts(&state)?.list()
}

#[tauri::command]
pub async fn get_draft(state: State<'_, AppState>, id: String) -> CommandResult<Draft> {
    drafts(&state)?.get(&id)
}

#[tauri::command]
pub async fn create_draft(state: State<'_, AppState>, content: DraftContent, step: DraftStep) -> CommandResult<Draft> {
    drafts(&state)?.create(content, step, &Now::local(), &state.operator_name)
}

/// Autosave. Rejected with a conflict if the draft changed since `revision`.
#[tauri::command]
pub async fn save_draft(
    state: State<'_, AppState>,
    id: String,
    revision: u64,
    content: DraftContent,
    step: DraftStep,
) -> CommandResult<Draft> {
    drafts(&state)?.save(&id, revision, content, step, &Now::local())
}

#[tauri::command]
pub async fn delete_draft(state: State<'_, AppState>, id: String, revision: u64) -> CommandResult<()> {
    drafts(&state)?.delete(&id, revision)
}

// ---- Investigation workflow -------------------------------------------------

/// Parses rows the operator copied in Excel and pasted into the app. The text
/// comes from the paste event only; the app has no clipboard permission.
#[tauri::command]
pub async fn parse_pasted_rows(text: String) -> CommandResult<PastedRows> {
    Ok(log_rows::parse_pasted_rows(&text)?)
}

#[tauri::command]
pub async fn review_draft(state: State<'_, AppState>, content: DraftContent) -> CommandResult<Review> {
    investigations(&state)?.review(&content, &Now::local())
}

/// Completes the saved draft: allocates the final number and publishes.
/// Idempotent: a draft that was already published returns its investigation.
#[tauri::command]
pub async fn complete_draft(state: State<'_, AppState>, id: String, revision: u64) -> CommandResult<Completion> {
    investigations(&state)?.complete(&id, revision, &Now::local(), &state.operator_name)
}

#[tauri::command]
pub async fn list_recent_investigations(state: State<'_, AppState>) -> CommandResult<Vec<InvestigationSummary>> {
    investigations(&state)?.recent(RECENT_LIMIT)
}

#[tauri::command]
pub async fn search_investigations(
    state: State<'_, AppState>,
    query: String,
) -> CommandResult<Vec<InvestigationSummary>> {
    investigations(&state)?.search(&query, &Now::local())
}

#[tauri::command]
pub async fn get_investigation(state: State<'_, AppState>, number: String) -> CommandResult<InvestigationDetails> {
    investigations(&state)?.get(parse_number(&number)?)
}

// ---- Distribution and export ------------------------------------------------

#[tauri::command]
pub async fn compose_distribution(state: State<'_, AppState>, number: String) -> CommandResult<DistributionMessage> {
    distribution(&state)?.compose(parse_number(&number)?)
}

#[tauri::command]
pub async fn distribute_investigation(state: State<'_, AppState>, number: String) -> CommandResult<SentDistribution> {
    distribution(&state)?.distribute(parse_number(&number)?, &Now::local(), &state.operator_name)
}

#[tauri::command]
pub async fn export_investigation_pdf(state: State<'_, AppState>, number: String) -> CommandResult<ExportedFile> {
    export(&state)?.export_investigation(parse_number(&number)?)
}

/// Draft export requires `confirm_incomplete` (the operator saw the warning).
#[tauri::command]
pub async fn export_draft_pdf(
    state: State<'_, AppState>,
    id: String,
    confirm_incomplete: bool,
) -> CommandResult<ExportedFile> {
    export(&state)?.export_draft(&id, confirm_incomplete)
}

// ---- Administration (admin mode only) ---------------------------------------

#[tauri::command]
pub async fn admin_get_configuration(state: State<'_, AppState>) -> CommandResult<AdminConfiguration> {
    state.admin()?.overview()
}

#[tauri::command]
pub async fn admin_save_system(state: State<'_, AppState>, input: SystemInput) -> CommandResult<System> {
    state.admin()?.save_system(&input)
}

#[tauri::command]
pub async fn admin_set_system_active(state: State<'_, AppState>, id: String, active: bool) -> CommandResult<System> {
    state.admin()?.set_system_active(&id, active)
}

#[tauri::command]
pub async fn admin_save_station(state: State<'_, AppState>, input: StationInput) -> CommandResult<Station> {
    state.admin()?.save_station(&input)
}

#[tauri::command]
pub async fn admin_set_station_active(state: State<'_, AppState>, id: String, active: bool) -> CommandResult<Station> {
    state.admin()?.set_station_active(&id, active)
}

#[tauri::command]
pub async fn admin_save_section(state: State<'_, AppState>, input: SectionInput) -> CommandResult<SectionDefinition> {
    state.admin()?.save_section(&input)
}

#[tauri::command]
pub async fn admin_set_section_active(
    state: State<'_, AppState>,
    id: String,
    active: bool,
) -> CommandResult<SectionDefinition> {
    state.admin()?.set_section_active(&id, active)
}

/// `offset`: -1 moves the section up, +1 down.
#[tauri::command]
pub async fn admin_move_section(state: State<'_, AppState>, id: String, offset: i32) -> CommandResult<Vec<String>> {
    state.admin()?.move_section(&id, offset)
}

#[tauri::command]
pub async fn admin_save_mail_template(state: State<'_, AppState>, input: MailTemplate) -> CommandResult<MailTemplate> {
    state.admin()?.save_mail_template(&input)
}

#[tauri::command]
pub async fn admin_save_publication(
    state: State<'_, AppState>,
    input: PublicationSettings,
) -> CommandResult<PublicationSettings> {
    state.admin()?.save_publication(&input)
}
