//! PDF export. Completed and distributed investigations export normally.
//! A draft exports only after the operator explicitly confirms that the
//! file may be incomplete, and the file is then marked as a draft on every
//! page.

use crate::adapters::json_file::safe_file_stem;
use crate::adapters::{ConfigurationRepository, DraftRepository, ExportSink, ExportedFile, InvestigationPublisher};
use crate::domain::draft::validate_draft_id;
use crate::domain::investigation_number::InvestigationNumber;
use crate::error::AppError;
use crate::render::document::DocumentView;
use crate::render::pdf::PdfRenderer;

pub struct ExportService<'a> {
    publisher: &'a dyn InvestigationPublisher,
    configuration: &'a dyn ConfigurationRepository,
    drafts: &'a dyn DraftRepository,
    sink: &'a dyn ExportSink,
}

impl<'a> ExportService<'a> {
    pub fn new(
        publisher: &'a dyn InvestigationPublisher,
        configuration: &'a dyn ConfigurationRepository,
        drafts: &'a dyn DraftRepository,
        sink: &'a dyn ExportSink,
    ) -> Self {
        Self { publisher, configuration, drafts, sink }
    }

    pub fn export_investigation(&self, number: InvestigationNumber) -> Result<ExportedFile, AppError> {
        let published = self.publisher.find(number)?.ok_or(AppError::NotFound)?;
        let view = DocumentView::from_investigation(&published.investigation, &published.lifecycle);
        let name = format!("{}.pdf", safe_file_stem(&format!("{number} - {}", published.investigation.activity.name)));
        Ok(self.sink.write(&name, &PdfRenderer::render(&view))?)
    }

    /// Exports a draft. Without `confirmed_incomplete` nothing is written and
    /// the caller is told that confirmation is required.
    pub fn export_draft(&self, id: &str, confirmed_incomplete: bool) -> Result<ExportedFile, AppError> {
        if !confirmed_incomplete {
            return Err(AppError::field("confirmIncomplete", "confirmation_required"));
        }
        validate_draft_id(id).map_err(|code| AppError::field("draftId", code))?;
        let draft = self.drafts.get(id)?.ok_or(AppError::NotFound)?;
        let configuration = self.configuration.load()?.value;
        let view = DocumentView::from_draft(&draft.content, &configuration);
        let activity = draft.content.activity.name.trim();
        let activity = if activity.is_empty() { crate::render::labels::UNNAMED } else { activity };
        let date = draft.updated_at.get(0..10).unwrap_or_default();
        let name = format!(
            "{}.pdf",
            safe_file_stem(&format!("{} - {activity} - {date}", crate::render::labels::DRAFT_FILE_PREFIX))
        );
        Ok(self.sink.write(&name, &PdfRenderer::render(&view))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::local_export::LocalFolderExport;
    use crate::services::investigations::tests::Fixture;
    use crate::services::testing::now;

    #[test]
    fn drafts_require_explicit_confirmation_and_are_marked() {
        let fixture = Fixture::new();
        let draft = fixture.draft();
        let folder = temp_dir("export");
        let sink = LocalFolderExport::new(folder.clone());
        let service = ExportService::new(&fixture.sharepoint, &fixture.base.configuration, &fixture.base.drafts, &sink);

        let refused = service.export_draft(&draft.id, false).unwrap_err();
        assert!(matches!(refused, AppError::Validation { ref errors } if errors[0].code == "confirmation_required"));
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0, "nothing written without confirmation");

        let file = service.export_draft(&draft.id, true).unwrap();
        assert_eq!(file.file_name, "טיוטה - בלט רומני - 2026-09-23.pdf");
        let bytes = std::fs::read(folder.join(&file.file_name)).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn completed_investigations_export_under_their_number() {
        let fixture = Fixture::new();
        let published = fixture.service().complete(&fixture.draft().id, 1, &now(), "op").unwrap();
        let folder = temp_dir("export");
        let sink = LocalFolderExport::new(folder.clone());
        let service = ExportService::new(&fixture.sharepoint, &fixture.base.configuration, &fixture.base.drafts, &sink);
        let file = service.export_investigation(published.investigation.number).unwrap();
        assert_eq!(file.file_name, "001-2026 - בלט רומני.pdf");
        assert!(matches!(
            service.export_investigation(InvestigationNumber::new(9, 2026).unwrap()),
            Err(AppError::NotFound)
        ));
    }
}
