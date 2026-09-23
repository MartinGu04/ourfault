//! Application state and composition root: this is the one place that
//! decides which adapter implementations are used.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::adapters::excel::RgbColor;
use crate::adapters::json_system_repository::JsonSystemRepository;
use crate::adapters::mock_distribution::MockDistribution;
use crate::adapters::mock_sharepoint::MockSharePoint;
use crate::adapters::{json_file, AdapterError, DistributionAdapter, SharePointAdapter, SystemRepository};
use crate::domain::operations_log::OperationsLog;
use crate::error::AppError;
use crate::seed;

/// The adapters behind the services. Replace the mock implementations here.
pub struct Backend {
    pub sharepoint: Box<dyn SharePointAdapter>,
    pub distribution: Box<dyn DistributionAdapter>,
    pub systems: Box<dyn SystemRepository>,
    pub settings: Settings,
}

impl Backend {
    pub fn open(data_dir: &Path) -> Result<Self, AdapterError> {
        Ok(Self {
            sharepoint: Box::new(MockSharePoint::open(
                data_dir.join("mock-sharepoint").join("investigations.json"),
                seed::investigations,
            )?),
            distribution: Box::new(MockDistribution::open(data_dir.join("mock-mail").join("outbox.json"))?),
            systems: Box::new(JsonSystemRepository::open(data_dir.join("systems.json"), seed::systems)?),
            settings: json_file::load_or_seed(&data_dir.join("settings.json"), Settings::default)?,
        })
    }
}

/// Local, per-workstation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Background colour (RRGGBB) that marks rows for pre-selection.
    pub excel_highlight_color: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { excel_highlight_color: "FFFF00".into() }
    }
}

impl Settings {
    pub fn highlight_color(&self) -> RgbColor {
        RgbColor::parse(&self.excel_highlight_color).unwrap_or_else(|| {
            crate::log_internal("settings", &"invalid excelHighlightColor, using FFFF00");
            RgbColor::parse("FFFF00").expect("valid default colour")
        })
    }
}

/// Who is using the application.
///
/// PoC only: the role comes from the `OURFAULT_DEMO_ROLE` environment variable
/// (`operator` or, by default, `admin`). A real deployment replaces this with
/// the Windows identity and a directory group lookup. Admin rights are
/// enforced by the commands, not only hidden in the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUser {
    pub display_name: String,
    pub is_admin: bool,
}

impl CurrentUser {
    pub fn from_environment() -> Self {
        match std::env::var("OURFAULT_DEMO_ROLE").as_deref() {
            Ok("operator") => Self { display_name: "מפעיל (הדגמה)".into(), is_admin: false },
            _ => Self { display_name: "מנהל מערכת (הדגמה)".into(), is_admin: true },
        }
    }
}

/// The workbook imported in this session. Kept on the Rust side so that
/// investigations are built from parsed data, not from rows sent back by the
/// webview.
pub struct CurrentImport {
    pub id: u64,
    pub log: OperationsLog,
}

pub struct AppState {
    backend: Option<Backend>,
    pub user: CurrentUser,
    current_import: Mutex<Option<CurrentImport>>,
    import_counter: AtomicU64,
}

impl AppState {
    /// A failed backend (e.g. unreadable data files) does not crash the app;
    /// every command then reports an internal error and the UI says so.
    pub fn new(backend: Option<Backend>, user: CurrentUser) -> Self {
        Self { backend, user, current_import: Mutex::new(None), import_counter: AtomicU64::new(0) }
    }

    pub fn backend(&self) -> Result<&Backend, AppError> {
        self.backend.as_ref().ok_or(AppError::Internal)
    }

    pub fn require_admin(&self) -> Result<(), AppError> {
        if self.user.is_admin {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// Replaces the current import and returns its id.
    pub fn set_import(&self, log: OperationsLog) -> Result<u64, AppError> {
        let id = self.import_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let mut current = self.current_import.lock().map_err(|_| AppError::Internal)?;
        *current = Some(CurrentImport { id, log });
        Ok(id)
    }

    /// Runs `action` with the imported log if `import_id` is still current.
    pub fn with_import<T>(
        &self,
        import_id: u64,
        action: impl FnOnce(&OperationsLog) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let current = self.current_import.lock().map_err(|_| AppError::Internal)?;
        match current.as_ref() {
            Some(import) if import.id == import_id => action(&import.log),
            _ => Err(AppError::field("rows", "import_expired")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::excel::read_operations_log;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::domain::investigation::InvestigationDraft;
    use crate::services::distribution::DistributionService;
    use crate::services::investigations::InvestigationService;
    use crate::services::systems::SystemService;
    use crate::services::testing::now;

    /// The PoC vertical slice against the real local adapters: import the demo
    /// workbook, create an investigation, distribute it, edit a system, then
    /// "restart" and check that everything was persisted.
    #[test]
    fn full_workflow_survives_restart() {
        let dir = temp_dir("workflow");
        let admin = CurrentUser { display_name: "admin".into(), is_admin: true };
        let state = AppState::new(Some(Backend::open(&dir).unwrap()), admin);
        let backend = state.backend().unwrap();

        let log = read_operations_log(seed::DEMO_WORKBOOK, "demo.xlsx", &backend.settings.highlight_color()).unwrap();
        let preselected: Vec<u32> = log.rows.iter().filter(|r| r.highlighted).map(|r| r.id).collect();
        let import_id = state.set_import(log).unwrap();

        let draft = InvestigationDraft {
            import_id,
            selected_row_ids: preselected.clone(),
            system_id: "system-1".into(),
            preliminary_check_url: "https://checks.example.com/runs/4471".into(),
        };
        let investigations = InvestigationService::new(backend.sharepoint.as_ref(), backend.systems.as_ref());
        let created = state.with_import(import_id, |log| investigations.create(&draft, log, &now(), "admin")).unwrap();
        assert_eq!(created.investigation.number.to_string(), "056-2026", "follows the seeded 055-2026");
        assert_eq!(created.investigation.rows.len(), preselected.len());

        let distribution = DistributionService::new(
            backend.sharepoint.as_ref(),
            backend.systems.as_ref(),
            backend.distribution.as_ref(),
        );
        let sent = distribution.distribute(created.investigation.number, &now()).unwrap();
        assert_eq!(sent.message.to.len(), 3);

        SystemService::new(backend.systems.as_ref()).set_active("system-2", false).unwrap();

        // A stale import id is rejected rather than silently using other rows.
        assert!(matches!(state.with_import(import_id + 1, |_| Ok(())), Err(AppError::Validation { .. })));

        drop(state);
        let reopened = Backend::open(&dir).unwrap();
        let found = reopened.sharepoint.find_investigation(created.investigation.number).unwrap();
        assert_eq!(found, Some(created));
        assert!(!reopened.systems.get("system-2").unwrap().unwrap().active);
        assert_eq!(reopened.settings.highlight_color(), RgbColor::parse("FFFF00").unwrap());
    }
}
