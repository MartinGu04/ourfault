//! Publishes investigations to a (shared) folder, as an alternative to
//! SharePoint:
//!
//! ```text
//! <root>/
//!   2026/
//!     056-2026.json               structured record (source of truth)
//!     056-2026 - בלט רומני.pdf    rendered document
//!   .drafts/
//!     <draft id>.marker           "this draft is published as 056-2026"
//!     <draft id>.lock             held while the draft is being published
//! ```
//!
//! The record file is created with create-if-absent semantics, which is
//! what reserves the number between workstations. Publishing a draft holds
//! that draft's lock file (also create-if-absent), so attempts for the same
//! draft run one after the other; inside the lock an already published
//! draft is detected and nothing is written. A lock left behind by a crash
//! is taken over once it is older than [`STALE_LOCK`]. The folder must already
//! exist: a missing or unreachable root is reported as unavailable and
//! nothing is written. Selected with `OURFAULT_PUBLISHER=folder`; not the
//! default, and not assumed to be the final production design.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use url::Url;

use super::json_file::{self, safe_file_stem};
use super::{AdapterError, InvestigationPublisher, PublishRequest};
use crate::domain::draft::validate_draft_id;
use crate::domain::investigation::{Publication, PublicationKind, PublishedInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::lifecycle::Lifecycle;
use crate::render::document::DocumentView;
use crate::render::pdf::PdfRenderer;

/// A draft lock older than this is left over from a crash.
const STALE_LOCK: Duration = Duration::from_secs(120);
/// How long an attempt waits for another attempt on the same draft.
const LOCK_WAIT: Duration = Duration::from_secs(10);

/// Held while a draft is being published; removed on drop.
struct DraftLock {
    path: PathBuf,
}

impl Drop for DraftLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub struct SharedFolderPublisher {
    root: PathBuf,
}

impl SharedFolderPublisher {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn check_root(&self) -> Result<(), AdapterError> {
        if self.root.is_dir() {
            Ok(())
        } else {
            Err(AdapterError::Unavailable(format!("{} is not reachable", self.root.display())))
        }
    }

    fn year_folder(&self, year: u16) -> PathBuf {
        self.root.join(year.to_string())
    }

    fn record_path(&self, number: InvestigationNumber) -> PathBuf {
        self.year_folder(number.year()).join(format!("{number}.json"))
    }

    fn records(&self) -> Result<Vec<PathBuf>, AdapterError> {
        self.check_root()?;
        let mut paths = Vec::new();
        for year in fs::read_dir(&self.root)? {
            let year = year?.path();
            if !year.is_dir() {
                continue;
            }
            for entry in fs::read_dir(&year)? {
                let path = entry?.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    paths.push(path);
                }
            }
        }
        Ok(paths)
    }

    /// Marker that ties a draft to the investigation it became.
    fn draft_marker(&self, draft_id: &str) -> Result<PathBuf, AdapterError> {
        validate_draft_id(draft_id).map_err(|code| AdapterError::Storage(format!("{code}: {draft_id:?}")))?;
        Ok(self.root.join(".drafts").join(format!("{draft_id}.marker")))
    }

    fn lock_draft(&self, draft_id: &str) -> Result<DraftLock, AdapterError> {
        let path = self.draft_marker(draft_id)?.with_extension("lock");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let started = std::time::Instant::now();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(DraftLock { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|modified| modified.elapsed().ok())
                        .is_some_and(|age| age > STALE_LOCK);
                    if stale {
                        let _ = fs::remove_file(&path);
                    } else if started.elapsed() > LOCK_WAIT {
                        return Err(AdapterError::Unavailable(format!(
                            "draft {draft_id} is being published elsewhere"
                        )));
                    } else {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn write_pdf(&self, published: &PublishedInvestigation, document: &DocumentView) -> Result<(), AdapterError> {
        let path = self.root.join(&published.publication.reference);
        json_file::write_atomic(&path, &PdfRenderer::render(document))
    }
}

fn pdf_name(published_number: InvestigationNumber, activity_name: &str) -> String {
    format!("{}.pdf", safe_file_stem(&format!("{published_number} - {activity_name}")))
}

fn file_url(path: &Path) -> String {
    Url::from_file_path(path).map(String::from).unwrap_or_else(|_| path.display().to_string())
}

impl InvestigationPublisher for SharedFolderPublisher {
    fn list(&self) -> Result<Vec<PublishedInvestigation>, AdapterError> {
        let mut all = Vec::new();
        for path in self.records()? {
            match json_file::load::<PublishedInvestigation>(&path) {
                Ok(Some(published)) => all.push(published),
                Ok(None) => {}
                Err(error) => crate::log_internal("reading published investigation", &error),
            }
        }
        Ok(all)
    }

    fn find(&self, number: InvestigationNumber) -> Result<Option<PublishedInvestigation>, AdapterError> {
        self.check_root()?;
        json_file::load(&self.record_path(number))
    }

    fn highest_number_in_year(&self, year: u16) -> Result<Option<InvestigationNumber>, AdapterError> {
        self.check_root()?;
        let folder = self.year_folder(year);
        if !folder.is_dir() {
            return Ok(None);
        }
        let mut highest = None;
        for entry in fs::read_dir(folder)? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if let Some(number) = name.strip_suffix(".json").and_then(|stem| stem.parse::<InvestigationNumber>().ok()) {
                highest = highest.max(Some(number));
            }
        }
        Ok(highest)
    }

    fn find_by_source_draft(&self, draft_id: &str) -> Result<Option<PublishedInvestigation>, AdapterError> {
        self.check_root()?;
        let is_source =
            |published: &PublishedInvestigation| published.investigation.source_draft_id.as_deref() == Some(draft_id);
        // Normally the marker names the investigation directly.
        let marker = self.draft_marker(draft_id)?;
        if let Some(number) = fs::read_to_string(&marker).ok().and_then(|text| text.trim().parse().ok()) {
            if let Some(published) = self.find(number)?.filter(is_source) {
                return Ok(Some(published));
            }
        }
        // Without a marker (e.g. interrupted after the record was written),
        // the records themselves say where each investigation came from.
        Ok(self.list()?.into_iter().find(is_source))
    }

    fn publish(&self, request: PublishRequest<'_>) -> Result<PublishedInvestigation, AdapterError> {
        self.check_root()?;
        let number = request.investigation.number;
        let source = request.investigation.source_draft_id.as_deref();
        // Attempts for the same draft run one after the other.
        let _lock = source.map(|id| self.lock_draft(id)).transpose()?;
        if let Some(id) = source {
            if self.find_by_source_draft(id)?.is_some() {
                return Err(AdapterError::DraftAlreadyPublished);
            }
        }

        let folder = self.year_folder(number.year());
        fs::create_dir_all(&folder)?;
        let pdf = pdf_name(number, &request.investigation.activity.name);
        let published = PublishedInvestigation {
            investigation: request.investigation.clone(),
            lifecycle: request.lifecycle.clone(),
            publication: Publication {
                destination: PublicationKind::SharedFolder,
                url: file_url(&folder.join(&pdf)),
                reference: format!("{}/{pdf}", number.year()),
            },
        };

        let record = self.record_path(number);
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&record) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(AdapterError::NumberTaken(number))
            }
            Err(error) => return Err(error.into()),
        };
        let written = json_file::to_versioned_json(&published)
            .and_then(|json| Ok(file.write_all(&json).and_then(|_| file.sync_all())?));
        drop(file);
        let marker = source.map(|id| self.draft_marker(id)).transpose();
        let finished = written.and_then(|_| self.write_pdf(&published, request.document)).and_then(|_| match marker {
            Ok(Some(marker)) => json_file::write_atomic(&marker, number.to_string().as_bytes()),
            Ok(None) => Ok(()),
            Err(error) => Err(error),
        });
        if let Err(error) = finished {
            // Release the number again: the investigation was not published.
            let _ = fs::remove_file(self.root.join(&published.publication.reference));
            let _ = fs::remove_file(&record);
            return Err(error);
        }
        Ok(published)
    }

    fn update_lifecycle(
        &self,
        number: InvestigationNumber,
        lifecycle: &Lifecycle,
        document: &DocumentView,
    ) -> Result<PublishedInvestigation, AdapterError> {
        let mut published =
            self.find(number)?.ok_or_else(|| AdapterError::Storage(format!("investigation {number} not found")))?;
        published.lifecycle = lifecycle.clone();
        json_file::save(&self.record_path(number), &published)?;
        self.write_pdf(&published, document)?;
        Ok(published)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::mock_sharepoint::tests::{investigation, publish};

    #[test]
    fn writes_a_record_and_a_pdf_per_investigation() {
        let root = temp_dir("folder");
        let publisher = SharedFolderPublisher::new(root.clone());
        let published = publish(&publisher, &investigation(56, 2026)).unwrap();

        assert_eq!(published.publication.reference, "2026/056-2026 - בלט רומני.pdf");
        assert!(published.publication.url.starts_with("file://"));
        let pdf = fs::read(root.join("2026").join("056-2026 - בלט רומני.pdf")).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert_eq!(publisher.find(published.investigation.number).unwrap(), Some(published));
        assert_eq!(publisher.highest_number_in_year(2026).unwrap().unwrap().to_string(), "056-2026");
        assert_eq!(publisher.list().unwrap().len(), 1);
    }

    #[test]
    fn a_draft_is_published_at_most_once() {
        let root = temp_dir("folder");
        let publisher = SharedFolderPublisher::new(root.clone());
        publish(&publisher, &investigation(56, 2026).with_source_draft("d-1")).unwrap();

        let error = publish(&publisher, &investigation(57, 2026).with_source_draft("d-1")).unwrap_err();
        assert!(matches!(error, AdapterError::DraftAlreadyPublished));
        assert_eq!(publisher.list().unwrap().len(), 1, "no second record");
        assert!(!root.join("2026").join("057-2026.json").exists(), "nothing was written");
        assert_eq!(publisher.highest_number_in_year(2026).unwrap().unwrap().to_string(), "056-2026");
        let found = publisher.find_by_source_draft("d-1").unwrap().unwrap();
        assert_eq!(found.investigation.number.to_string(), "056-2026");
        assert_eq!(publisher.find_by_source_draft("d-2").unwrap(), None);
    }

    #[test]
    fn a_lock_left_by_a_crash_is_taken_over() {
        let publisher = SharedFolderPublisher::new(temp_dir("folder"));
        let lock = publisher.draft_marker("d-1").unwrap().with_extension("lock");
        fs::create_dir_all(lock.parent().unwrap()).unwrap();
        let file = fs::File::create(&lock).unwrap();
        file.set_modified(std::time::SystemTime::now() - STALE_LOCK - Duration::from_secs(1)).unwrap();
        drop(file);
        publish(&publisher, &investigation(1, 2026).with_source_draft("d-1")).unwrap();
        assert!(!lock.exists(), "released after publishing");
    }

    #[test]
    fn finds_the_source_even_without_a_marker() {
        let publisher = SharedFolderPublisher::new(temp_dir("folder"));
        publish(&publisher, &investigation(56, 2026).with_source_draft("d-1")).unwrap();
        fs::remove_file(publisher.draft_marker("d-1").unwrap()).unwrap();
        let found = publisher.find_by_source_draft("d-1").unwrap().unwrap();
        assert_eq!(found.investigation.number.to_string(), "056-2026");
    }

    #[test]
    fn a_taken_number_is_reported_as_a_conflict() {
        let publisher = SharedFolderPublisher::new(temp_dir("folder"));
        publish(&publisher, &investigation(7, 2026)).unwrap();
        assert!(matches!(publish(&publisher, &investigation(7, 2026)), Err(AdapterError::NumberTaken(_))));
    }

    #[test]
    fn an_unreachable_folder_writes_nothing() {
        let root = temp_dir("folder").join("share-not-mounted");
        let publisher = SharedFolderPublisher::new(root.clone());
        assert!(matches!(publish(&publisher, &investigation(1, 2026)), Err(AdapterError::Unavailable(_))));
        assert!(matches!(publisher.highest_number_in_year(2026), Err(AdapterError::Unavailable(_))));
        assert!(!root.exists());
    }
}
