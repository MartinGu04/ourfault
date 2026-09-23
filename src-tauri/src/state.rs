//! Application state and composition root: this is the one place that
//! decides which adapter implementations are used.

use std::path::Path;

use serde::Serialize;

use crate::adapters::json_system_repository::JsonSystemRepository;
use crate::adapters::mock_distribution::MockDistribution;
use crate::adapters::mock_sharepoint::MockSharePoint;
use crate::adapters::{AdapterError, DistributionAdapter, SharePointAdapter, SystemRepository};
use crate::error::AppError;
use crate::seed;

/// The adapters behind the services. Replace the mock implementations here.
pub struct Backend {
    pub sharepoint: Box<dyn SharePointAdapter>,
    pub distribution: Box<dyn DistributionAdapter>,
    pub systems: Box<dyn SystemRepository>,
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

pub struct AppState {
    backend: Option<Backend>,
    pub user: CurrentUser,
}

impl AppState {
    /// A failed backend (e.g. unreadable data files) does not crash the app;
    /// every command then reports an internal error and the UI says so.
    pub fn new(backend: Option<Backend>, user: CurrentUser) -> Self {
        Self { backend, user }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::domain::investigation::InvestigationDraft;
    use crate::domain::log_rows::parse_pasted_rows;
    use crate::services::distribution::DistributionService;
    use crate::services::investigations::InvestigationService;
    use crate::services::systems::SystemService;
    use crate::services::testing::now;

    /// The PoC vertical slice against the local adapters: paste rows, remove
    /// one during review, create an investigation, distribute it, edit a
    /// system, then "restart" and check that everything was persisted.
    #[test]
    fn full_workflow_survives_restart() {
        let dir = temp_dir("workflow");
        let backend = Backend::open(&dir).unwrap();

        let pasted = parse_pasted_rows(seed::DEMO_PASTE).unwrap();
        let mut rows = pasted.rows;
        rows.remove(2); // the operator removes an unrelated row (08:22) while reviewing

        let draft = InvestigationDraft {
            system_id: "system-1".into(),
            preliminary_check_url: "https://checks.example.com/runs/4471".into(),
            rows: rows.clone(),
        };
        let investigations = InvestigationService::new(backend.sharepoint.as_ref(), backend.systems.as_ref());
        let created = investigations.create(&draft, &now(), "admin").unwrap();
        assert_eq!(created.investigation.number.to_string(), "056-2026", "follows the seeded 055-2026");
        assert_eq!(created.investigation.rows, rows);
        assert!(created.url.contains("DispForm.aspx?ID="));

        let distribution = DistributionService::new(
            backend.sharepoint.as_ref(),
            backend.systems.as_ref(),
            backend.distribution.as_ref(),
        );
        let sent = distribution.distribute(created.investigation.number, &now()).unwrap();
        assert_eq!(sent.message.to.len(), 3);
        assert!(sent.message.body.contains(&created.url));

        SystemService::new(backend.systems.as_ref()).set_active("system-2", false).unwrap();

        drop(backend);
        let reopened = Backend::open(&dir).unwrap();
        let found = reopened.sharepoint.find_investigation(created.investigation.number).unwrap();
        assert_eq!(found, Some(created));
        assert!(!reopened.systems.get("system-2").unwrap().unwrap().active);
    }
}
