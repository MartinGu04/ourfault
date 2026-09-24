//! Stand-in for SharePoint. Simulates the investigations list in a local
//! JSON file and makes no network requests; the URLs it returns are
//! fictional.
//!
//! Each list item shows both mappings a real adapter may need:
//! * `fields` – the dashboard columns (number, systems, activity, date, ...),
//! * `investigation` – the full structured investigation, for lists that
//!   expose separate fields,
//! * `bodyHtml` – the investigation rendered as HTML, for a list whose form
//!   has one rich-text body field.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::{json_file, AdapterError, InvestigationPublisher, PublishRequest};
use crate::domain::configuration::PublicationSettings;
use crate::domain::investigation::{
    Investigation, InvestigationSummary, Publication, PublicationKind, PublishedInvestigation,
};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::lifecycle::Lifecycle;
use crate::render::document::DocumentView;
use crate::render::html::HtmlRenderer;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListItem {
    item_id: u32,
    url: String,
    fields: InvestigationSummary,
    investigation: Investigation,
    lifecycle: Lifecycle,
    body_html: String,
}

impl ListItem {
    fn published(&self) -> PublishedInvestigation {
        PublishedInvestigation {
            investigation: self.investigation.clone(),
            lifecycle: self.lifecycle.clone(),
            publication: Publication {
                destination: PublicationKind::SharePoint,
                url: self.url.clone(),
                reference: self.item_id.to_string(),
            },
        }
    }
}

pub struct MockSharePoint {
    path: PathBuf,
    // The mutex makes create-if-absent atomic within this process, mirroring
    // the uniqueness guarantee the real list must provide.
    items: Mutex<Vec<ListItem>>,
}

impl MockSharePoint {
    /// Opens the list at `path`, seeding it on first run.
    pub fn open(path: PathBuf, seed: impl FnOnce() -> Vec<PublishedInvestigation>) -> Result<Self, AdapterError> {
        let items = json_file::load_or_seed(&path, || {
            seed()
                .into_iter()
                .map(|published| ListItem {
                    item_id: published.publication.reference.parse().unwrap_or(0),
                    url: published.publication.url.clone(),
                    fields: InvestigationSummary::from(&published),
                    body_html: HtmlRenderer::render_fragment(&DocumentView::from_investigation(
                        &published.investigation,
                        &published.lifecycle,
                    )),
                    investigation: published.investigation,
                    lifecycle: published.lifecycle,
                })
                .collect()
        })?;
        Ok(Self { path, items: Mutex::new(items) })
    }

    fn items(&self) -> Result<std::sync::MutexGuard<'_, Vec<ListItem>>, AdapterError> {
        self.items.lock().map_err(|_| AdapterError::Storage("mock SharePoint lock poisoned".into()))
    }

    /// Fictional address of the item's form in the configured list.
    fn item_url(target: &PublicationSettings, item_id: u32) -> String {
        format!(
            "{}/Lists/{}/DispForm.aspx?ID={item_id}",
            target.site_url.trim_end_matches('/'),
            target.list.replace(' ', "%20")
        )
    }

    #[cfg(test)]
    pub fn body_html(&self, number: InvestigationNumber) -> Option<String> {
        self.items().ok()?.iter().find(|item| item.investigation.number == number).map(|item| item.body_html.clone())
    }

    /// Writes `items`; on failure the in-memory list is left unchanged.
    fn commit(&self, items: &mut Vec<ListItem>, updated: Vec<ListItem>) -> Result<(), AdapterError> {
        json_file::save(&self.path, &updated)?;
        *items = updated;
        Ok(())
    }
}

impl InvestigationPublisher for MockSharePoint {
    fn list(&self) -> Result<Vec<PublishedInvestigation>, AdapterError> {
        Ok(self.items()?.iter().map(ListItem::published).collect())
    }

    fn find(&self, number: InvestigationNumber) -> Result<Option<PublishedInvestigation>, AdapterError> {
        Ok(self.items()?.iter().find(|item| item.investigation.number == number).map(ListItem::published))
    }

    fn highest_number_in_year(&self, year: u16) -> Result<Option<InvestigationNumber>, AdapterError> {
        Ok(self.items()?.iter().map(|item| item.investigation.number).filter(|n| n.year() == year).max())
    }

    fn publish(&self, request: PublishRequest<'_>) -> Result<PublishedInvestigation, AdapterError> {
        let mut items = self.items()?;
        let number = request.investigation.number;
        if items.iter().any(|item| item.investigation.number == number) {
            return Err(AdapterError::NumberTaken(number));
        }
        let item_id = items.iter().map(|item| item.item_id).max().unwrap_or(0) + 1;
        let url = Self::item_url(request.target, item_id);
        let published = PublishedInvestigation {
            investigation: request.investigation.clone(),
            lifecycle: request.lifecycle.clone(),
            publication: Publication {
                destination: PublicationKind::SharePoint,
                url: url.clone(),
                reference: item_id.to_string(),
            },
        };
        let item = ListItem {
            item_id,
            url,
            fields: InvestigationSummary::from(&published),
            investigation: published.investigation.clone(),
            lifecycle: published.lifecycle.clone(),
            body_html: HtmlRenderer::render_fragment(request.document),
        };
        let mut updated = items.clone();
        updated.push(item);
        self.commit(&mut items, updated)?;
        Ok(published)
    }

    fn update_lifecycle(
        &self,
        number: InvestigationNumber,
        lifecycle: &Lifecycle,
        document: &DocumentView,
    ) -> Result<PublishedInvestigation, AdapterError> {
        let mut items = self.items()?;
        let mut updated = items.clone();
        let item = updated
            .iter_mut()
            .find(|item| item.investigation.number == number)
            .ok_or_else(|| AdapterError::Storage(format!("investigation {number} not found")))?;
        item.lifecycle = lifecycle.clone();
        item.fields.status = lifecycle.status;
        item.body_html = HtmlRenderer::render_fragment(document);
        let published = item.published();
        self.commit(&mut items, updated)?;
        Ok(published)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::domain::configuration::tests::configuration;
    use crate::domain::investigation::tests::{content, number};
    use crate::domain::lifecycle::InvestigationStatus;

    pub(crate) fn investigation(sequence: u32, year: u16) -> Investigation {
        Investigation::build(number(), &content(), &configuration())
            .unwrap()
            .with_number(InvestigationNumber::new(sequence, year).unwrap())
    }

    pub(crate) fn publish(
        publisher: &dyn InvestigationPublisher,
        investigation: &Investigation,
    ) -> Result<PublishedInvestigation, AdapterError> {
        let lifecycle = Lifecycle::completed("2026-09-23T10:00:00+03:00", "tester");
        let document = DocumentView::from_investigation(investigation, &lifecycle);
        publisher.publish(PublishRequest {
            investigation,
            lifecycle: &lifecycle,
            document: &document,
            target: &configuration().publication,
        })
    }

    fn store() -> MockSharePoint {
        MockSharePoint::open(temp_dir("sp").join("sp.json"), Vec::new).unwrap()
    }

    #[test]
    fn creates_list_items_with_dashboard_fields_and_html_body() {
        let store = store();
        let published = publish(&store, &investigation(56, 2026)).unwrap();
        assert_eq!(published.publication.reference, "1");
        assert_eq!(
            published.publication.url,
            "https://sharepoint.example.com/sites/ops/Lists/Investigations/DispForm.aspx?ID=1"
        );
        assert_eq!(publish(&store, &investigation(57, 2026)).unwrap().publication.reference, "2");

        let number = published.investigation.number;
        assert_eq!(store.find(number).unwrap(), Some(published));
        assert!(store.body_html(number).unwrap().contains("<h1>תחקיר 056-2026</h1>"));
    }

    #[test]
    fn rejects_duplicate_numbers() {
        let store = store();
        publish(&store, &investigation(56, 2026)).unwrap();
        let error = publish(&store, &investigation(56, 2026)).unwrap_err();
        assert!(matches!(error, AdapterError::NumberTaken(n) if n.to_string() == "056-2026"));
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn reports_the_highest_number_per_year() {
        let store = store();
        for (sequence, year) in [(412, 2025), (9, 2026), (1000, 2026)] {
            publish(&store, &investigation(sequence, year)).unwrap();
        }
        assert_eq!(store.highest_number_in_year(2026).unwrap().unwrap().to_string(), "1000-2026");
        assert_eq!(store.highest_number_in_year(2025).unwrap().unwrap().to_string(), "412-2025");
        assert_eq!(store.highest_number_in_year(2027).unwrap(), None);
    }

    #[test]
    fn updates_the_lifecycle_and_persists_across_restarts() {
        let path = temp_dir("sp").join("sp.json");
        let number = {
            let store = MockSharePoint::open(path.clone(), Vec::new).unwrap();
            let published = publish(&store, &investigation(1, 2026)).unwrap();
            let mut lifecycle = published.lifecycle.clone();
            lifecycle.record_distribution("2026-09-23T11:00:00+03:00", "tester").unwrap();
            let document = DocumentView::from_investigation(&published.investigation, &lifecycle);
            store.update_lifecycle(published.investigation.number, &lifecycle, &document).unwrap();
            published.investigation.number
        };
        let reopened = MockSharePoint::open(path, || panic!("must not reseed")).unwrap();
        let found = reopened.find(number).unwrap().unwrap();
        assert_eq!(found.lifecycle.status, InvestigationStatus::Distributed);
        assert!(reopened.body_html(number).unwrap().contains("הופץ"));
    }
}
