//! Application state and composition root: this is the one place that
//! decides which adapter implementations are used, and it holds the
//! current work mode.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::adapters::local_configuration::LocalConfigurationRepository;
use crate::adapters::local_drafts::LocalDraftRepository;
use crate::adapters::local_export::LocalFolderExport;
use crate::adapters::mock_distribution::MockDistribution;
use crate::adapters::mock_sharepoint::MockSharePoint;
use crate::adapters::shared_folder::SharedFolderPublisher;
use crate::adapters::{
    AdapterError, ConfigurationRepository, DistributionAdapter, DraftRepository, ExportSink, InvestigationPublisher,
};
use crate::error::AppError;
use crate::seed;
use crate::services::configuration::Administration;

/// Which publication destination to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublisherChoice {
    /// Default: the local SharePoint simulation.
    MockSharePoint,
    /// A (shared) folder that must already exist.
    SharedFolder(PathBuf),
}

pub struct BackendOptions {
    pub data_dir: PathBuf,
    pub export_dir: PathBuf,
    pub publisher: PublisherChoice,
}

/// The adapters behind the services. Replace the mock implementations here.
pub struct Backend {
    pub configuration: Box<dyn ConfigurationRepository>,
    pub drafts: Box<dyn DraftRepository>,
    pub publisher: Box<dyn InvestigationPublisher>,
    pub distribution: Box<dyn DistributionAdapter>,
    pub exports: Box<dyn ExportSink>,
}

impl Backend {
    /// Opens (and on first run seeds) the local data. The file names are
    /// new in schema version 2; files from PR #1 are left untouched and
    /// ignored (see docs/ARCHITECTURE.md).
    pub fn open(options: &BackendOptions) -> Result<Self, AdapterError> {
        let dir = &options.data_dir;
        let publisher: Box<dyn InvestigationPublisher> = match &options.publisher {
            PublisherChoice::MockSharePoint => Box::new(MockSharePoint::open(
                dir.join("mock-sharepoint").join("investigations-v2.json"),
                seed::investigations,
            )?),
            PublisherChoice::SharedFolder(root) => Box::new(SharedFolderPublisher::new(root.clone())),
        };
        Ok(Self {
            configuration: Box::new(LocalConfigurationRepository::open(
                dir.join("configuration.json"),
                seed::configuration,
            )?),
            drafts: Box::new(LocalDraftRepository::open_with_seed(dir.join("drafts"), seed::drafts)?),
            publisher,
            distribution: Box::new(MockDistribution::open(dir.join("mock-mail").join("outbox-v2.json"))?),
            exports: Box::new(LocalFolderExport::new(options.export_dir.clone())),
        })
    }
}

/// The two ways of working with OurFault. This is a UX mode, not an
/// authenticated account: choosing admin mode does not prove who the user
/// is. Whether a mode may be entered is decided by an [`AccessPolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkMode {
    /// Creating, viewing and distributing investigations.
    Regular,
    /// Initial setup and configuration: systems, stations, sections, mail.
    Admin,
}

/// Decides which work modes are available. The PoC policy allows admin mode
/// unless `OURFAULT_ADMIN_MODE=disabled`. A real deployment replaces it with
/// environment permissions (e.g. a directory group of the Windows user)
/// without changing any command.
pub trait AccessPolicy: Send + Sync {
    fn allows(&self, mode: WorkMode) -> bool;
}

pub struct DemoAccessPolicy {
    pub admin_enabled: bool,
}

impl DemoAccessPolicy {
    pub fn from_environment() -> Self {
        Self { admin_enabled: std::env::var("OURFAULT_ADMIN_MODE").as_deref() != Ok("disabled") }
    }
}

impl AccessPolicy for DemoAccessPolicy {
    fn allows(&self, mode: WorkMode) -> bool {
        mode == WorkMode::Regular || self.admin_enabled
    }
}

/// Proof that admin mode was checked. It has a private field, so only this
/// module can create one ([`AppState::admin`]); administration services
/// require it.
pub struct AdminGrant {
    _private: (),
}

impl AdminGrant {
    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self { _private: () }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// `None` until a mode is chosen on the entry screen.
    pub mode: Option<WorkMode>,
    pub admin_available: bool,
    /// Recorded as author of drafts and status changes.
    pub operator_name: String,
}

pub struct AppState {
    backend: Option<Backend>,
    policy: Box<dyn AccessPolicy>,
    mode: Mutex<Option<WorkMode>>,
    pub operator_name: String,
}

impl AppState {
    /// A failed backend (e.g. unreadable data files) does not crash the app;
    /// every command then reports an internal error and the UI says so.
    pub fn new(backend: Option<Backend>, policy: Box<dyn AccessPolicy>, operator_name: String) -> Self {
        Self { backend, policy, mode: Mutex::new(None), operator_name }
    }

    pub fn backend(&self) -> Result<&Backend, AppError> {
        self.backend.as_ref().ok_or(AppError::Internal)
    }

    fn current_mode(&self) -> Option<WorkMode> {
        self.mode.lock().map(|mode| *mode).unwrap_or(None)
    }

    pub fn session(&self) -> Session {
        Session {
            mode: self.current_mode(),
            admin_available: self.policy.allows(WorkMode::Admin),
            operator_name: self.operator_name.clone(),
        }
    }

    pub fn enter(&self, mode: WorkMode) -> Result<Session, AppError> {
        if !self.policy.allows(mode) {
            return Err(AppError::Forbidden);
        }
        *self.mode.lock().map_err(|_| AppError::Internal)? = Some(mode);
        Ok(self.session())
    }

    /// Back to the entry screen.
    pub fn leave(&self) -> Session {
        if let Ok(mut mode) = self.mode.lock() {
            *mode = None;
        }
        self.session()
    }

    /// Administration services, only in admin mode. Every admin command
    /// goes through here.
    pub fn admin(&self) -> Result<Administration<'_>, AppError> {
        let in_admin_mode = self.current_mode() == Some(WorkMode::Admin);
        if !in_admin_mode || !self.policy.allows(WorkMode::Admin) {
            return Err(AppError::Forbidden);
        }
        Ok(Administration::new(self.backend()?.configuration.as_ref(), AdminGrant { _private: () }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::domain::draft::DraftStep;
    use crate::domain::lifecycle::InvestigationStatus;
    use crate::domain::log_rows::parse_pasted_rows;
    use crate::domain::station::StationInput;
    use crate::services::distribution::DistributionService;
    use crate::services::drafts::DraftService;
    use crate::services::export::ExportService;
    use crate::services::investigations::InvestigationService;
    use crate::services::testing::{now, now_at};

    fn options(dir: &std::path::Path) -> BackendOptions {
        BackendOptions {
            data_dir: dir.join("data"),
            export_dir: dir.join("exports"),
            publisher: PublisherChoice::MockSharePoint,
        }
    }

    fn state(dir: &std::path::Path, admin_enabled: bool) -> AppState {
        AppState::new(
            Some(Backend::open(&options(dir)).unwrap()),
            Box::new(DemoAccessPolicy { admin_enabled }),
            "op".into(),
        )
    }

    #[test]
    fn regular_mode_cannot_reach_administration() {
        let dir = temp_dir("modes");
        let state = state(&dir, true);
        assert_eq!(state.session().mode, None);
        assert!(matches!(state.admin(), Err(AppError::Forbidden)), "no mode chosen yet");

        state.enter(WorkMode::Regular).unwrap();
        assert!(matches!(state.admin(), Err(AppError::Forbidden)));

        state.enter(WorkMode::Admin).unwrap();
        let admin = state.admin().unwrap();
        let station = admin.save_station(&StationInput { id: None, name: "תחנת ערבה".into() }).unwrap();
        assert!(admin.current().unwrap().stations.contains(&station));

        state.leave();
        assert!(matches!(state.admin(), Err(AppError::Forbidden)), "leaving admin mode revokes access");
    }

    #[test]
    fn the_access_policy_can_withhold_admin_mode() {
        let dir = temp_dir("modes");
        let state = state(&dir, false);
        assert!(!state.session().admin_available);
        assert!(matches!(state.enter(WorkMode::Admin), Err(AppError::Forbidden)));
        assert_eq!(state.enter(WorkMode::Regular).unwrap().mode, Some(WorkMode::Regular));
    }

    /// The vertical slice against the local adapters: draft with autosave,
    /// pasted chronology, completion with the next number, distribution,
    /// export, then a "restart" to check everything was persisted.
    #[test]
    fn full_workflow_survives_restart() {
        let dir = temp_dir("workflow");
        let backend = Backend::open(&options(&dir)).unwrap();
        let drafts = DraftService::new(backend.drafts.as_ref(), backend.configuration.as_ref());
        let investigations = InvestigationService::new(
            backend.publisher.as_ref(),
            backend.configuration.as_ref(),
            backend.drafts.as_ref(),
        );

        // Reopen the seeded draft and continue where the operator stopped.
        let seeded = drafts.list().unwrap().remove(0);
        assert_eq!(seeded.step, DraftStep::Technical);
        let draft = drafts.get(&seeded.id).unwrap();

        let mut content = draft.content.clone();
        let mut rows = parse_pasted_rows(seed::DEMO_PASTE).unwrap().rows;
        rows.remove(2); // the operator removes an unrelated row while reviewing
        content.rows = rows.clone();
        content.preliminary_check_url = "https://checks.example.com/runs/4480".into();
        content.sections.insert(
            "section-3".into(),
            vec![serde_json::from_str(r#"{"f1": "18", "f2": false, "f4": "system-2"}"#).unwrap()],
        );
        let saved = drafts.save(&draft.id, draft.revision, content, DraftStep::Review, &now_at("10:30")).unwrap();

        let review = investigations.review(&saved.content, &now()).unwrap();
        assert_eq!(review.issues, vec![], "{:?}", review.issues);
        assert_eq!(review.expected_number.unwrap().to_string(), "056-2026", "follows the seeded 055-2026");

        let created = investigations.complete(&saved.id, saved.revision, &now(), "op").unwrap();
        assert_eq!(created.investigation.number.to_string(), "056-2026");
        assert_eq!(created.investigation.rows, rows);
        assert_eq!(created.investigation.activity.systems.len(), 3);
        assert!(created.publication.url.contains("DispForm.aspx?ID=47"));

        let distribution = DistributionService::new(
            backend.publisher.as_ref(),
            backend.configuration.as_ref(),
            backend.distribution.as_ref(),
        );
        let sent = distribution.distribute(created.investigation.number, &now_at("11:00"), "op").unwrap();
        assert_eq!(
            sent.message.to,
            vec!["alpha-ops@example.com", "ops-center@example.com", "beta-ops@example.com", "gamma-tech@example.com"],
            "union of three systems without the duplicate address"
        );

        let export = ExportService::new(
            backend.publisher.as_ref(),
            backend.configuration.as_ref(),
            backend.drafts.as_ref(),
            backend.exports.as_ref(),
        );
        let file = export.export_investigation(created.investigation.number).unwrap();
        assert!(dir.join("exports").join(&file.file_name).exists());

        drop(backend);
        let reopened = Backend::open(&options(&dir)).unwrap();
        let found = reopened.publisher.find(created.investigation.number).unwrap().unwrap();
        assert_eq!(found.lifecycle.status, InvestigationStatus::Distributed);
        let drafts = DraftService::new(reopened.drafts.as_ref(), reopened.configuration.as_ref());
        assert!(drafts.list().unwrap().is_empty(), "the converted draft is gone from the list");
    }

    #[test]
    fn files_from_the_previous_schema_are_left_alone() {
        let dir = temp_dir("legacy");
        let data = dir.join("data");
        std::fs::create_dir_all(data.join("mock-sharepoint")).unwrap();
        std::fs::write(data.join("systems.json"), "[]").unwrap();
        std::fs::write(data.join("mock-sharepoint").join("investigations.json"), "[]").unwrap();
        Backend::open(&options(&dir)).unwrap();
        assert_eq!(std::fs::read_to_string(data.join("systems.json")).unwrap(), "[]");
        assert!(data.join("configuration.json").exists());
    }
}
