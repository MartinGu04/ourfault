//! Boundaries to everything outside the application: configuration and
//! draft storage, publication destinations, e-mail and exported files.
//!
//! The rest of the application depends only on the traits in this file. The
//! PoC ships local/mock implementations; `state.rs` chooses which ones are
//! used, so real ones (shared storage, SharePoint, Outlook) can replace them
//! without touching the services or the UI.

pub(crate) mod json_file;
pub mod local_configuration;
pub mod local_drafts;
pub mod local_export;
pub mod mock_distribution;
pub mod mock_sharepoint;
pub mod shared_folder;

use crate::domain::configuration::Configuration;
use crate::domain::draft::Draft;
use crate::domain::investigation::{Investigation, PublishedInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::lifecycle::Lifecycle;
use crate::domain::mail::{DistributionMessage, DistributionReceipt};
use crate::render::document::DocumentView;

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// The investigation number was taken by someone else in the meantime.
    #[error("investigation number {0} already exists")]
    NumberTaken(InvestigationNumber),
    /// The stored item changed since it was read (optimistic concurrency).
    #[error("the stored item was changed by someone else")]
    Conflict,
    /// The destination cannot be reached (network share, SharePoint...).
    /// Nothing was written. The message is for logs only.
    #[error("destination unavailable: {0}")]
    Unavailable(String),
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

/// A value together with the revision it was read at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revisioned<T> {
    pub value: T,
    pub revision: u64,
}

/// Administrator configuration. A single versioned document: rarely
/// changed, read by every workstation. Local JSON in the PoC; shared
/// storage later.
pub trait ConfigurationRepository: Send + Sync {
    fn load(&self) -> Result<Revisioned<Configuration>, AdapterError>;

    /// Stores `configuration` if the stored revision is still
    /// `expected_revision`, and returns the new revision. Otherwise fails
    /// with [`AdapterError::Conflict`] and changes nothing.
    fn save(&self, configuration: &Configuration, expected_revision: u64) -> Result<u64, AdapterError>;
}

/// Investigations in progress. Local JSON files in the PoC. The real
/// implementation is expected to be shared (network) storage, which is why
/// every write is guarded by the draft's revision.
pub trait DraftRepository: Send + Sync {
    fn list(&self) -> Result<Vec<Draft>, AdapterError>;

    fn get(&self, id: &str) -> Result<Option<Draft>, AdapterError>;

    /// Writes `draft`. With `expected_revision == None` the draft must not
    /// exist yet; otherwise the stored draft must still be at
    /// `expected_revision`. Fails with [`AdapterError::Conflict`] if not.
    fn save(&self, draft: &Draft, expected_revision: Option<u64>) -> Result<(), AdapterError>;

    /// Deletes a draft that is still at `expected_revision`.
    fn delete(&self, id: &str, expected_revision: u64) -> Result<(), AdapterError>;
}

/// What a publisher receives. The structured investigation stays the source
/// of truth; `document` is the neutral view the publisher renders into
/// whatever format its destination needs (HTML for a SharePoint rich-text
/// field, PDF for a shared folder).
pub struct PublishRequest<'a> {
    pub investigation: &'a Investigation,
    pub lifecycle: &'a Lifecycle,
    pub document: &'a DocumentView,
    /// SharePoint target from the configuration (ignored by other publishers).
    pub target: &'a crate::domain::configuration::PublicationSettings,
}

/// The destination of record for completed investigations: SharePoint in
/// production, a shared folder as a possible fallback.
pub trait InvestigationPublisher: Send + Sync {
    fn list(&self) -> Result<Vec<PublishedInvestigation>, AdapterError>;

    fn find(&self, number: InvestigationNumber) -> Result<Option<PublishedInvestigation>, AdapterError>;

    /// Highest existing number in `year`, used to propose the next number.
    fn highest_number_in_year(&self, year: u16) -> Result<Option<InvestigationNumber>, AdapterError>;

    /// Publishes the investigation under `investigation.number`.
    ///
    /// Implementations MUST make this an atomic create-if-absent and return
    /// [`AdapterError::NumberTaken`] if the number already exists (for
    /// SharePoint: a unique-values column or an ETag-guarded counter item).
    /// The service retries with the next number, which is what makes
    /// concurrent creation from several workstations safe. If the
    /// destination cannot be reached, nothing may be written and
    /// [`AdapterError::Unavailable`] is returned.
    fn publish(&self, request: PublishRequest<'_>) -> Result<PublishedInvestigation, AdapterError>;

    /// Records a new lifecycle (e.g. after distribution) for an existing
    /// investigation and refreshes its rendered output.
    fn update_lifecycle(
        &self,
        number: InvestigationNumber,
        lifecycle: &Lifecycle,
        document: &DocumentView,
    ) -> Result<PublishedInvestigation, AdapterError>;
}

/// Sends distribution e-mail. In production: as the signed-in Outlook user,
/// with their signature.
pub trait DistributionAdapter: Send + Sync {
    fn send(&self, message: &DistributionMessage, sent_at: &str) -> Result<DistributionReceipt, AdapterError>;
}

/// A file written for the operator.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedFile {
    pub file_name: String,
    pub folder: String,
}

/// Where exported files go. The webview never chooses a path.
pub trait ExportSink: Send + Sync {
    /// Writes a new file, never overwriting an existing one (a suffix is
    /// added instead). `file_name` must already be safe.
    fn write(&self, file_name: &str, bytes: &[u8]) -> Result<ExportedFile, AdapterError>;
}
