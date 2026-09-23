//! Boundaries to everything outside the application: SharePoint, the mail
//! system and configuration storage.
//!
//! The rest of the application depends only on the traits in this file. The
//! PoC ships local/mock implementations; real ones can replace them in
//! `lib.rs` without touching the services or the UI.

pub(crate) mod json_file;
pub mod json_system_repository;
pub mod mock_distribution;
pub mod mock_sharepoint;

use crate::domain::distribution::{DistributionMessage, DistributionReceipt};
use crate::domain::investigation::{Investigation, StoredInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::system::{SharePointDestination, System};

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// The investigation number was taken by someone else in the meantime.
    #[error("investigation number {0} already exists")]
    NumberTaken(InvestigationNumber),
    /// Storage or transport failure. The message is for logs only and must
    /// never be shown to users.
    #[error("storage failure: {0}")]
    Storage(String),
}

impl From<std::io::Error> for AdapterError {
    fn from(error: std::io::Error) -> Self {
        AdapterError::Storage(error.to_string())
    }
}

/// Creates and reads investigations. In production each investigation is an
/// editable SharePoint list item (web form), and SharePoint is the source of
/// truth once it exists: OurFault keeps no copy and always reads back from
/// the adapter.
pub trait SharePointAdapter: Send + Sync {
    fn list_investigations(&self) -> Result<Vec<StoredInvestigation>, AdapterError>;

    fn find_investigation(&self, number: InvestigationNumber) -> Result<Option<StoredInvestigation>, AdapterError>;

    /// Highest existing number in `year`, used to propose the next number.
    fn highest_number_in_year(&self, year: u16) -> Result<Option<InvestigationNumber>, AdapterError>;

    /// Creates the investigation item under `investigation.number` and returns
    /// it with its SharePoint item id and URL.
    ///
    /// Implementations MUST make this an atomic create-if-absent and return
    /// [`AdapterError::NumberTaken`] if the number already exists (for real
    /// SharePoint: a unique-values column or an ETag-guarded counter item).
    /// The service layer retries with the next number, which is what makes
    /// concurrent creation from several workstations safe.
    fn create_investigation(
        &self,
        investigation: Investigation,
        destination: &SharePointDestination,
        created_by: &str,
        created_at: &str,
    ) -> Result<StoredInvestigation, AdapterError>;
}

/// Sends investigation notifications. E-mail in production.
pub trait DistributionAdapter: Send + Sync {
    fn send(&self, message: &DistributionMessage, sent_at: &str) -> Result<DistributionReceipt, AdapterError>;
}

/// Persists system configuration. Local JSON in the PoC; shared storage later.
pub trait SystemRepository: Send + Sync {
    fn list(&self) -> Result<Vec<System>, AdapterError>;

    fn get(&self, id: &str) -> Result<Option<System>, AdapterError> {
        Ok(self.list()?.into_iter().find(|system| system.id == id))
    }

    /// Inserts a new system or replaces the one with the same id.
    fn save(&self, system: System) -> Result<(), AdapterError>;
}
