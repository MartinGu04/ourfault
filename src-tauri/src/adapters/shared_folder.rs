//! Publishes investigations to a (shared) folder, as an alternative to
//! SharePoint:
//!
//! ```text
//! <root>/
//!   2026/
//!     056-2026.json               structured record (source of truth)
//!     056-2026 - בלט רומני.pdf    rendered document
//! ```
//!
//! The record file is created with create-if-absent semantics, which is
//! what reserves the number between workstations. The folder must already
//! exist: a missing or unreachable root is reported as unavailable and
//! nothing is written. Selected with `OURFAULT_PUBLISHER=folder`; not the
//! default, and not assumed to be the final production design.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use url::Url;

use super::json_file::{self, safe_file_stem};
use super::{AdapterError, InvestigationPublisher, PublishRequest};
use crate::domain::investigation::{Publication, PublicationKind, PublishedInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::lifecycle::Lifecycle;
use crate::render::document::DocumentView;
use crate::render::pdf::PdfRenderer;

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

    fn publish(&self, request: PublishRequest<'_>) -> Result<PublishedInvestigation, AdapterError> {
        self.check_root()?;
        let number = request.investigation.number;
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
            .and_then(|json| Ok(file.write_all(&json).and_then(|_| file.sync_all())?))
            .and_then(|_| self.write_pdf(&published, request.document));
        if let Err(error) = written {
            // Release the number again: the investigation was not published.
            drop(file);
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
