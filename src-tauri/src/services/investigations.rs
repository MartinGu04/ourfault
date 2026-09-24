use serde::Serialize;

use super::Now;
use crate::adapters::{AdapterError, ConfigurationRepository, DraftRepository, InvestigationPublisher, PublishRequest};
use crate::domain::draft::{Conversion, DraftContent};
use crate::domain::investigation::{Investigation, InvestigationSummary, PublishedInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::lifecycle::Lifecycle;
use crate::domain::validation::FieldError;
use crate::error::AppError;
use crate::render::document::DocumentView;

/// How often completion retries with a fresh number after a conflict.
const MAX_ALLOCATION_ATTEMPTS: usize = 5;
const SEARCH_LIMIT: usize = 30;

/// The review step: what is still missing, and the document as it stands.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    /// Empty when the draft can be completed.
    pub issues: Vec<FieldError>,
    /// Always marked as a draft.
    pub document: DocumentView,
    /// The number the investigation would get now. Informational only, and
    /// `None` when the destination cannot be reached.
    pub expected_number: Option<InvestigationNumber>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationDetails {
    pub investigation: PublishedInvestigation,
    pub summary: InvestigationSummary,
    pub document: DocumentView,
}

pub struct InvestigationService<'a> {
    publisher: &'a dyn InvestigationPublisher,
    configuration: &'a dyn ConfigurationRepository,
    drafts: &'a dyn DraftRepository,
}

impl<'a> InvestigationService<'a> {
    pub fn new(
        publisher: &'a dyn InvestigationPublisher,
        configuration: &'a dyn ConfigurationRepository,
        drafts: &'a dyn DraftRepository,
    ) -> Self {
        Self { publisher, configuration, drafts }
    }

    /// The number the next investigation is expected to get. The number is
    /// only reserved, atomically, by [`Self::complete`].
    pub fn next_number(&self, now: &Now) -> Result<InvestigationNumber, AppError> {
        let year = now.year()?;
        let highest = self.publisher.highest_number_in_year(year)?;
        InvestigationNumber::next_after(highest, year).map_err(|e| AppError::internal("next number", &e))
    }

    /// Validates draft content without saving or publishing anything.
    pub fn review(&self, content: &DraftContent, now: &Now) -> Result<Review, AppError> {
        let configuration = self.configuration.load()?.value;
        let expected_number = match self.next_number(now) {
            Ok(number) => Some(number),
            Err(AppError::Unavailable) => None,
            Err(other) => return Err(other),
        };
        let candidate = expected_number
            .unwrap_or(InvestigationNumber::new(1, now.year()?).map_err(|e| AppError::internal("number", &e))?);
        let issues = Investigation::build(candidate, content, &configuration).err().unwrap_or_default();
        Ok(Review { issues, document: DocumentView::from_draft(content, &configuration), expected_number })
    }

    /// Completes a draft: validates it, allocates the final number and
    /// publishes it, then marks the draft as converted.
    ///
    /// Numbering is optimistic: take the next free number, ask the publisher
    /// to create-if-absent, and on a conflict (another workstation was
    /// faster) retry with a fresh number. If anything fails before the
    /// publisher confirms, the draft is left untouched and no number is used.
    pub fn complete(
        &self,
        draft_id: &str,
        expected_revision: u64,
        now: &Now,
        by: &str,
    ) -> Result<PublishedInvestigation, AppError> {
        let draft = self.drafts.get(draft_id)?.ok_or(AppError::NotFound)?;
        if !draft.is_open() {
            return Err(AppError::Conflict { code: "draft_converted" });
        }
        if draft.revision != expected_revision {
            return Err(AppError::Conflict { code: "changed" });
        }
        let configuration = self.configuration.load()?.value;
        let mut candidate = self.next_number(now)?;
        let investigation =
            Investigation::build(candidate, &draft.content, &configuration).map_err(AppError::validation)?;
        let lifecycle = Lifecycle::completed(&now.timestamp, by);

        for _ in 0..MAX_ALLOCATION_ATTEMPTS {
            let attempt = investigation.clone().with_number(candidate);
            let document = DocumentView::from_investigation(&attempt, &lifecycle);
            let request = PublishRequest {
                investigation: &attempt,
                lifecycle: &lifecycle,
                document: &document,
                target: &configuration.publication,
            };
            match self.publisher.publish(request) {
                Ok(published) => {
                    let converted = crate::domain::draft::Draft {
                        revision: draft.revision + 1,
                        updated_at: now.timestamp.clone(),
                        converted: Some(Conversion {
                            number: published.investigation.number,
                            at: now.timestamp.clone(),
                        }),
                        ..draft
                    };
                    // The investigation exists now; a failure here must not
                    // turn the successful publication into an error.
                    if let Err(error) = self.drafts.save(&converted, Some(expected_revision)) {
                        crate::log_internal("marking draft as converted", &error);
                    }
                    return Ok(published);
                }
                Err(AdapterError::NumberTaken(taken)) => {
                    // Prefer the destination's view; never retry the same number.
                    let fresh = self.next_number(now)?;
                    candidate = if fresh > taken {
                        fresh
                    } else {
                        InvestigationNumber::next_after(Some(taken), taken.year())
                            .map_err(|e| AppError::internal("next number", &e))?
                    };
                }
                Err(other) => return Err(other.into()),
            }
        }
        Err(AppError::internal("complete investigation", &"could not allocate a free investigation number"))
    }

    /// Most recent investigations first.
    pub fn recent(&self, limit: usize) -> Result<Vec<InvestigationSummary>, AppError> {
        let mut all: Vec<InvestigationSummary> =
            self.publisher.list()?.iter().map(InvestigationSummary::from).collect();
        all.sort_by_key(|summary| std::cmp::Reverse(summary.number));
        all.truncate(limit);
        Ok(all)
    }

    /// Matches the investigation number (`056-2026`, `56-2026`, or `56` for
    /// the current year), activity name or system name. No full-text index:
    /// a simple scan of the list.
    pub fn search(&self, query: &str, now: &Now) -> Result<Vec<InvestigationSummary>, AppError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let exact = InvestigationNumber::parse_lenient(query, now.year()?).ok();
        let mut matches: Vec<InvestigationSummary> = self
            .publisher
            .list()?
            .iter()
            .map(InvestigationSummary::from)
            .filter(|summary| Some(summary.number) == exact || summary.matches_text(query))
            .collect();
        matches.sort_by_key(|summary| (Some(summary.number) != exact, std::cmp::Reverse(summary.number)));
        matches.truncate(SEARCH_LIMIT);
        Ok(matches)
    }

    pub fn get(&self, number: InvestigationNumber) -> Result<InvestigationDetails, AppError> {
        let investigation = self.publisher.find(number)?.ok_or(AppError::NotFound)?;
        Ok(InvestigationDetails {
            summary: InvestigationSummary::from(&investigation),
            document: DocumentView::from_investigation(&investigation.investigation, &investigation.lifecycle),
            investigation,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::mock_sharepoint::MockSharePoint;
    use crate::adapters::shared_folder::SharedFolderPublisher;
    use crate::domain::draft::DraftStep;
    use crate::domain::investigation::tests::content;
    use crate::domain::lifecycle::InvestigationStatus;
    use crate::services::drafts::tests::Fixture as DraftFixture;
    use crate::services::testing::now;

    pub(crate) struct Fixture {
        pub base: DraftFixture,
        pub sharepoint: MockSharePoint,
    }

    impl Fixture {
        pub(crate) fn new() -> Self {
            Self {
                base: DraftFixture::new(),
                sharepoint: MockSharePoint::open(temp_dir("sp").join("sp.json"), Vec::new).unwrap(),
            }
        }

        pub(crate) fn service(&self) -> InvestigationService<'_> {
            self.with_publisher(&self.sharepoint)
        }

        pub(crate) fn with_publisher<'s>(
            &'s self,
            publisher: &'s dyn InvestigationPublisher,
        ) -> InvestigationService<'s> {
            InvestigationService::new(publisher, &self.base.configuration, &self.base.drafts)
        }

        /// A saved draft with complete content.
        pub(crate) fn draft(&self) -> crate::domain::draft::Draft {
            self.base.service().create(content(), DraftStep::Review, &now(), "op").unwrap()
        }
    }

    #[test]
    fn a_draft_does_not_consume_a_number() {
        let fixture = Fixture::new();
        let service = fixture.service();
        fixture.draft();
        fixture.draft();
        let review = service.review(&content(), &now()).unwrap();
        assert!(review.issues.is_empty());
        assert_eq!(review.expected_number.unwrap().to_string(), "001-2026");
        assert!(fixture.sharepoint.list().unwrap().is_empty(), "nothing published");
        assert!(review.document.draft_notice.is_some());
    }

    #[test]
    fn review_lists_what_is_missing() {
        let fixture = Fixture::new();
        let mut incomplete = content();
        incomplete.activity.activity_type = None;
        incomplete.rows.clear();
        let review = fixture.service().review(&incomplete, &now()).unwrap();
        let fields: Vec<&str> = review.issues.iter().map(|i| i.field.as_str()).collect();
        assert_eq!(fields, vec!["activityType", "rows"]);
    }

    #[test]
    fn completion_allocates_consecutive_numbers_and_converts_the_draft() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let first = fixture.draft();
        let second = fixture.draft();

        let a = service.complete(&first.id, 1, &now(), "op").unwrap();
        let b = service.complete(&second.id, 1, &now(), "op").unwrap();
        assert_eq!(a.investigation.number.to_string(), "001-2026");
        assert_eq!(b.investigation.number.to_string(), "002-2026");
        assert_eq!(a.lifecycle.status, InvestigationStatus::Completed);
        assert_eq!(a.lifecycle.completed_by, "op");

        let stored = fixture.base.drafts.get(&first.id).unwrap().unwrap();
        assert_eq!(stored.converted.unwrap().number, a.investigation.number);
        assert!(fixture.base.service().list().unwrap().is_empty(), "converted drafts leave the drafts list");
        let again = service.complete(&first.id, 2, &now(), "op").unwrap_err();
        assert!(matches!(again, AppError::Conflict { code: "draft_converted" }), "no double completion");
        assert_eq!(service.recent(10).unwrap()[0].number, b.investigation.number);
    }

    #[test]
    fn incomplete_drafts_are_not_completed() {
        let fixture = Fixture::new();
        let mut draft = fixture.draft();
        let mut incomplete = draft.content.clone();
        incomplete.activity.system_ids.clear();
        draft = fixture.base.service().save(&draft.id, 1, incomplete, DraftStep::Review, &now()).unwrap();
        let error = fixture.service().complete(&draft.id, draft.revision, &now(), "op").unwrap_err();
        assert!(matches!(error, AppError::Validation { ref errors } if errors[0].field == "systemIds"));
        assert!(fixture.sharepoint.list().unwrap().is_empty());
        assert!(fixture.base.drafts.get(&draft.id).unwrap().unwrap().is_open());
    }

    #[test]
    fn failed_publication_keeps_the_draft_and_consumes_no_number() {
        let fixture = Fixture::new();
        let draft = fixture.draft();
        let offline = SharedFolderPublisher::new(temp_dir("share").join("not-mounted"));
        let error = fixture.with_publisher(&offline).complete(&draft.id, 1, &now(), "op").unwrap_err();
        assert!(matches!(error, AppError::Unavailable));

        let kept = fixture.base.drafts.get(&draft.id).unwrap().unwrap();
        assert!(kept.is_open());
        assert_eq!(kept, draft, "the draft is untouched");

        // Once the destination is back, the same draft gets the first number.
        let created = fixture.service().complete(&draft.id, 1, &now(), "op").unwrap();
        assert_eq!(created.investigation.number.to_string(), "001-2026");
    }

    #[test]
    fn review_works_while_the_destination_is_unavailable() {
        let fixture = Fixture::new();
        let offline = SharedFolderPublisher::new(temp_dir("share").join("not-mounted"));
        let review = fixture.with_publisher(&offline).review(&content(), &now()).unwrap();
        assert_eq!(review.expected_number, None);
        assert!(review.issues.is_empty());
    }

    /// Simulates another workstation publishing between our number lookup
    /// and our create.
    struct RacingPublisher {
        inner: MockSharePoint,
        raced: AtomicBool,
    }

    impl InvestigationPublisher for RacingPublisher {
        fn list(&self) -> Result<Vec<PublishedInvestigation>, AdapterError> {
            self.inner.list()
        }
        fn find(&self, n: InvestigationNumber) -> Result<Option<PublishedInvestigation>, AdapterError> {
            self.inner.find(n)
        }
        fn highest_number_in_year(&self, year: u16) -> Result<Option<InvestigationNumber>, AdapterError> {
            self.inner.highest_number_in_year(year)
        }
        fn publish(&self, request: PublishRequest<'_>) -> Result<PublishedInvestigation, AdapterError> {
            if !self.raced.swap(true, Ordering::SeqCst) {
                let competitor = Lifecycle::completed("t", "other workstation");
                self.inner.publish(PublishRequest { lifecycle: &competitor, ..request })?;
            }
            self.inner.publish(request)
        }
        fn update_lifecycle(
            &self,
            number: InvestigationNumber,
            lifecycle: &Lifecycle,
            document: &DocumentView,
        ) -> Result<PublishedInvestigation, AdapterError> {
            self.inner.update_lifecycle(number, lifecycle, document)
        }
    }

    #[test]
    fn concurrent_completion_retries_with_the_next_free_number() {
        let fixture = Fixture::new();
        let racing = RacingPublisher {
            inner: MockSharePoint::open(temp_dir("race").join("sp.json"), Vec::new).unwrap(),
            raced: AtomicBool::new(false),
        };
        let service = fixture.with_publisher(&racing);
        let draft = fixture.draft();

        let expected = service.review(&draft.content, &now()).unwrap().expected_number.unwrap();
        let created = service.complete(&draft.id, 1, &now(), "op").unwrap();

        assert_eq!(expected.to_string(), "001-2026");
        assert_eq!(created.investigation.number.to_string(), "002-2026");
        let owners: Vec<String> = racing.list().unwrap().into_iter().map(|p| p.lifecycle.completed_by).collect();
        assert_eq!(owners, vec!["other workstation", "op"]);
    }

    #[test]
    fn searches_by_number_activity_and_system() {
        let fixture = Fixture::new();
        let service = fixture.service();
        service.complete(&fixture.draft().id, 1, &now(), "op").unwrap();
        let mut other = content();
        other.activity.name = "ניסוי שקט".into();
        other.activity.system_ids = vec!["alpha".into()];
        let draft = fixture.base.service().create(other, DraftStep::Review, &now(), "op").unwrap();
        service.complete(&draft.id, 1, &now(), "op").unwrap();

        let numbers = |query: &str| -> Vec<String> {
            service.search(query, &now()).unwrap().into_iter().map(|s| s.number.to_string()).collect()
        };
        assert_eq!(numbers("2"), vec!["002-2026", "001-2026"], "exact number first, then text matches");
        assert_eq!(numbers("56-2026"), Vec::<String>::new());
        assert_eq!(numbers("רומני"), vec!["001-2026"]);
        assert_eq!(numbers("מערכת beta"), vec!["001-2026"]);
        assert_eq!(numbers("alpha"), vec!["002-2026", "001-2026"]);
        assert!(numbers(" ").is_empty());

        let details = service.get("001-2026".parse().unwrap()).unwrap();
        assert_eq!(details.document.title, "תחקיר 001-2026");
        assert!(matches!(service.get("009-2026".parse().unwrap()), Err(AppError::NotFound)));
    }
}
