use serde::Serialize;

use super::Now;
use crate::adapters::{AdapterError, ConfigurationRepository, DraftRepository, InvestigationPublisher, PublishRequest};
use crate::domain::advisories::{assess_activity, Advisory};
use crate::domain::draft::{Conversion, Draft, DraftContent};
use crate::domain::investigation::{Investigation, InvestigationSummary, PublishedInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::lifecycle::Lifecycle;
use crate::error::AppError;
use crate::render::document::DocumentView;

/// How often completion retries with a fresh number after a conflict.
const MAX_ALLOCATION_ATTEMPTS: usize = 5;
const SEARCH_LIMIT: usize = 30;

/// The review step: what blocks completion, what deserves attention, and the
/// document as it stands.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    /// Errors first (they block completion), then warnings and information.
    /// The draft can be completed when there is no error.
    pub advisories: Vec<Advisory>,
    /// Always marked as a draft.
    pub document: DocumentView,
    /// The number the investigation would get now. Informational only, and
    /// `None` when the destination cannot be reached.
    pub expected_number: Option<InvestigationNumber>,
}

/// The result of completing a draft.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Completion {
    pub investigation: PublishedInvestigation,
    /// True when the draft had already been published (e.g. a retry after a
    /// partial failure): the existing investigation is returned and nothing
    /// new was created.
    pub already_existed: bool,
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
        let errors = Investigation::build(candidate, content, &configuration).err().unwrap_or_default();
        let mut advisories: Vec<Advisory> = errors.into_iter().map(Advisory::from).collect();
        advisories.extend(assess_activity(&content.activity, &configuration.night_window));
        Ok(Review { advisories, document: DocumentView::from_draft(content, &configuration), expected_number })
    }

    /// Completes a draft: validates it, allocates the final number and
    /// publishes it, then marks the draft as converted.
    ///
    /// Idempotent: one draft creates at most one investigation. The publisher
    /// is asked first whether this draft was already published (this does
    /// not rely on the draft's "converted" mark, whose write may have failed
    /// after publication), and the publisher itself refuses a second
    /// investigation from the same draft, so even simultaneous attempts
    /// return the one existing investigation.
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
    ) -> Result<Completion, AppError> {
        let draft = self.drafts.get(draft_id)?.ok_or(AppError::NotFound)?;
        if let Some(existing) = self.publisher.find_by_source_draft(&draft.id)? {
            return Ok(self.already_created(draft, existing, now));
        }
        if !draft.is_open() {
            return Err(AppError::Conflict { code: "draft_converted" });
        }
        if draft.revision != expected_revision {
            return Err(AppError::Conflict { code: "changed" });
        }
        let configuration = self.configuration.load()?.value;
        let mut candidate = self.next_number(now)?;
        let investigation = Investigation::build(candidate, &draft.content, &configuration)
            .map_err(AppError::validation)?
            .with_source_draft(&draft.id);
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
                    self.mark_converted(draft, &published, now);
                    return Ok(Completion { investigation: published, already_existed: false });
                }
                Err(AdapterError::DraftAlreadyPublished) => {
                    // A simultaneous attempt for the same draft won.
                    let existing = self
                        .publisher
                        .find_by_source_draft(&draft.id)?
                        .ok_or_else(|| AppError::internal("complete investigation", &"published draft not found"))?;
                    return Ok(self.already_created(draft, existing, now));
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

    fn already_created(&self, draft: Draft, existing: PublishedInvestigation, now: &Now) -> Completion {
        if draft.is_open() {
            self.mark_converted(draft, &existing, now);
        }
        Completion { investigation: existing, already_existed: true }
    }

    /// Records on the draft which investigation it became. Best effort: the
    /// investigation exists already, and a failure here is repaired by the
    /// next completion attempt, which finds the investigation by its source
    /// draft.
    fn mark_converted(&self, draft: Draft, published: &PublishedInvestigation, now: &Now) {
        let expected = draft.revision;
        let converted = Draft {
            revision: expected + 1,
            updated_at: now.timestamp.clone(),
            converted: Some(Conversion { number: published.investigation.number, at: now.timestamp.clone() }),
            ..draft
        };
        if let Err(error) = self.drafts.save(&converted, Some(expected)) {
            crate::log_internal("marking draft as converted", &error);
        }
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
    use crate::domain::advisories::Severity;
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

    fn errors(review: &Review) -> Vec<&Advisory> {
        review.advisories.iter().filter(|a| a.severity == Severity::Error).collect()
    }

    #[test]
    fn warnings_and_information_do_not_block_completion() {
        let fixture = Fixture::new();
        let mut content = content();
        // Crosses midnight but answered "not a night activity"; started early.
        content.activity.planned_start = "2026-09-20T20:00".into();
        content.activity.planned_end = "2026-09-21T02:00".into();
        content.activity.actual_start = "2026-09-20T19:00".into();
        content.activity.actual_end = "2026-09-21T01:30".into();
        content.activity.night_activity = Some(false);
        let review = fixture.service().review(&content, &now()).unwrap();
        let found: Vec<(Severity, &str)> = review.advisories.iter().map(|a| (a.severity, a.code)).collect();
        assert_eq!(
            found,
            vec![(Severity::Warning, "night_overlap_not_marked"), (Severity::Info, "actual_started_before_plan")]
        );

        let draft = fixture.base.service().create(content, DraftStep::Review, &now(), "op").unwrap();
        let created = fixture.service().complete(&draft.id, 1, &now(), "op").unwrap();
        assert!(!created.investigation.investigation.activity.night_activity, "the answer is never changed");
    }

    #[test]
    fn errors_come_first_and_block_completion() {
        let fixture = Fixture::new();
        let mut content = content();
        content.activity.planned_end = "2026-09-20T07:00".into();
        content.activity.night_activity = Some(false);
        content.activity.actual_end = "2026-09-20T21:00".into();
        let review = fixture.service().review(&content, &now()).unwrap();
        assert_eq!(review.advisories[0].severity, Severity::Error);
        assert_eq!(review.advisories[0].code, "end_before_start");
        assert!(review.advisories.iter().any(|a| a.severity == Severity::Warning));
        let draft = fixture.base.service().create(content, DraftStep::Review, &now(), "op").unwrap();
        assert!(matches!(fixture.service().complete(&draft.id, 1, &now(), "op"), Err(AppError::Validation { .. })));
    }

    #[test]
    fn a_draft_does_not_consume_a_number() {
        let fixture = Fixture::new();
        let service = fixture.service();
        fixture.draft();
        fixture.draft();
        let review = service.review(&content(), &now()).unwrap();
        assert!(errors(&review).is_empty());
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
        let fields: Vec<&str> = errors(&review).iter().map(|i| i.field.as_str()).collect();
        assert_eq!(fields, vec!["activityType", "rows"]);
    }

    #[test]
    fn completion_allocates_consecutive_numbers_and_converts_the_draft() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let first = fixture.draft();
        let second = fixture.draft();

        let a = service.complete(&first.id, 1, &now(), "op").unwrap().investigation;
        let b = service.complete(&second.id, 1, &now(), "op").unwrap().investigation;
        assert_eq!(a.investigation.number.to_string(), "001-2026");
        assert_eq!(b.investigation.number.to_string(), "002-2026");
        assert_eq!(a.lifecycle.status, InvestigationStatus::Completed);
        assert_eq!(a.lifecycle.completed_by, "op");

        let stored = fixture.base.drafts.get(&first.id).unwrap().unwrap();
        assert_eq!(stored.converted.unwrap().number, a.investigation.number);
        assert!(fixture.base.service().list().unwrap().is_empty(), "converted drafts leave the drafts list");
        assert_eq!(a.investigation.source_draft_id.as_deref(), Some(first.id.as_str()));
        let again = service.complete(&first.id, 2, &now(), "op").unwrap();
        assert!(again.already_existed, "completing again returns the same investigation");
        assert_eq!(again.investigation, a);
        assert_eq!(fixture.sharepoint.list().unwrap().len(), 2);
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
        assert!(!created.already_existed);
        assert_eq!(created.investigation.investigation.number.to_string(), "001-2026");
    }

    #[test]
    fn review_works_while_the_destination_is_unavailable() {
        let fixture = Fixture::new();
        let offline = SharedFolderPublisher::new(temp_dir("share").join("not-mounted"));
        let review = fixture.with_publisher(&offline).review(&content(), &now()).unwrap();
        assert_eq!(review.expected_number, None);
        assert!(errors(&review).is_empty());
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
        fn find_by_source_draft(&self, id: &str) -> Result<Option<PublishedInvestigation>, AdapterError> {
            self.inner.find_by_source_draft(id)
        }
        fn publish(&self, request: PublishRequest<'_>) -> Result<PublishedInvestigation, AdapterError> {
            if !self.raced.swap(true, Ordering::SeqCst) {
                // Another workstation's investigation (from another draft).
                let competitor = Lifecycle::completed("t", "other workstation");
                let mut other = request.investigation.clone();
                other.source_draft_id = Some("d-other".into());
                self.inner.publish(PublishRequest { investigation: &other, lifecycle: &competitor, ..request })?;
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
        let created = service.complete(&draft.id, 1, &now(), "op").unwrap().investigation;

        assert_eq!(expected.to_string(), "001-2026");
        assert_eq!(created.investigation.number.to_string(), "002-2026");
        let owners: Vec<String> = racing.list().unwrap().into_iter().map(|p| p.lifecycle.completed_by).collect();
        assert_eq!(owners, vec!["other workstation", "op"]);
    }

    /// Draft storage whose write of the "converted" mark fails once
    /// (e.g. the share dropped right after publication succeeded).
    struct ConversionFails<'a> {
        inner: &'a dyn DraftRepository,
        fail: AtomicBool,
    }

    impl DraftRepository for ConversionFails<'_> {
        fn list(&self) -> Result<Vec<Draft>, AdapterError> {
            self.inner.list()
        }
        fn get(&self, id: &str) -> Result<Option<Draft>, AdapterError> {
            self.inner.get(id)
        }
        fn save(&self, draft: &Draft, expected: Option<u64>) -> Result<(), AdapterError> {
            if draft.converted.is_some() && self.fail.swap(false, Ordering::SeqCst) {
                return Err(AdapterError::Storage("network share went away".into()));
            }
            self.inner.save(draft, expected)
        }
        fn delete(&self, id: &str, expected: u64) -> Result<(), AdapterError> {
            self.inner.delete(id, expected)
        }
    }

    fn check_partial_failure_recovery(publisher: &dyn InvestigationPublisher, fixture: &Fixture) {
        let drafts = ConversionFails { inner: &fixture.base.drafts, fail: AtomicBool::new(true) };
        let service = InvestigationService::new(publisher, &fixture.base.configuration, &drafts);
        let draft = fixture.draft();

        // Publication succeeds, marking the draft converted fails.
        let first = service.complete(&draft.id, 1, &now(), "op").unwrap();
        assert!(!first.already_existed);
        let number = first.investigation.investigation.number;
        assert_eq!(number.to_string(), "001-2026");
        let stored = fixture.base.drafts.get(&draft.id).unwrap().unwrap();
        assert!(stored.is_open(), "the converted mark was not written");

        // The operator retries (same revision, as the UI still has it).
        let retry = service.complete(&draft.id, 1, &now(), "op").unwrap();
        assert!(retry.already_existed);
        assert_eq!(retry.investigation.investigation.number, number, "no new number");
        assert_eq!(publisher.list().unwrap().len(), 1, "no second publisher record");
        assert_eq!(publisher.highest_number_in_year(2026).unwrap(), Some(number), "no number consumed");
        let repaired = fixture.base.drafts.get(&draft.id).unwrap().unwrap();
        assert_eq!(repaired.converted.map(|c| c.number), Some(number), "the retry repairs the draft");

        // Even later attempts (any revision) keep returning the same one.
        let later = service.complete(&draft.id, 99, &now(), "op").unwrap();
        assert!(later.already_existed);
        assert_eq!(later.investigation.investigation.number, number);
        assert_eq!(publisher.list().unwrap().len(), 1);
    }

    #[test]
    fn retry_after_partial_failure_returns_the_existing_sharepoint_item() {
        let fixture = Fixture::new();
        check_partial_failure_recovery(&fixture.sharepoint, &fixture);
    }

    #[test]
    fn retry_after_partial_failure_returns_the_existing_folder_record() {
        let fixture = Fixture::new();
        let folder = SharedFolderPublisher::new(temp_dir("share"));
        check_partial_failure_recovery(&folder, &fixture);
    }

    fn check_simultaneous_completion(publisher: &dyn InvestigationPublisher, fixture: &Fixture) {
        let service = fixture.with_publisher(publisher);
        let draft = fixture.draft();
        let results: Vec<Completion> = std::thread::scope(|scope| {
            let attempts: Vec<_> =
                (0..4).map(|_| scope.spawn(|| service.complete(&draft.id, 1, &now(), "op").unwrap())).collect();
            attempts.into_iter().map(|attempt| attempt.join().unwrap()).collect()
        });
        let numbers: std::collections::BTreeSet<String> =
            results.iter().map(|r| r.investigation.investigation.number.to_string()).collect();
        assert_eq!(numbers.len(), 1, "every attempt returns the same investigation: {numbers:?}");
        assert_eq!(results.iter().filter(|r| !r.already_existed).count(), 1, "exactly one attempt created it");
        let from_draft: Vec<PublishedInvestigation> = publisher
            .list()
            .unwrap()
            .into_iter()
            .filter(|p| p.investigation.source_draft_id.as_deref() == Some(draft.id.as_str()))
            .collect();
        assert_eq!(from_draft.len(), 1, "one official investigation per draft");
    }

    #[test]
    fn simultaneous_completions_create_one_sharepoint_item() {
        let fixture = Fixture::new();
        check_simultaneous_completion(&fixture.sharepoint, &fixture);
    }

    #[test]
    fn simultaneous_completions_create_one_folder_record() {
        let fixture = Fixture::new();
        let folder = SharedFolderPublisher::new(temp_dir("share"));
        check_simultaneous_completion(&folder, &fixture);
    }

    /// A publisher whose lookup does not (yet) see the earlier publication,
    /// like a lagging list index: the publisher's own guard must still stop
    /// the second investigation.
    struct LaggingLookup {
        inner: MockSharePoint,
        lookups: std::sync::atomic::AtomicUsize,
    }

    impl InvestigationPublisher for LaggingLookup {
        fn list(&self) -> Result<Vec<PublishedInvestigation>, AdapterError> {
            self.inner.list()
        }
        fn find(&self, n: InvestigationNumber) -> Result<Option<PublishedInvestigation>, AdapterError> {
            self.inner.find(n)
        }
        fn highest_number_in_year(&self, year: u16) -> Result<Option<InvestigationNumber>, AdapterError> {
            self.inner.highest_number_in_year(year)
        }
        fn find_by_source_draft(&self, id: &str) -> Result<Option<PublishedInvestigation>, AdapterError> {
            // The pre-publication checks of both attempts see nothing.
            if self.lookups.fetch_add(1, Ordering::SeqCst) < 2 {
                return Ok(None);
            }
            self.inner.find_by_source_draft(id)
        }
        fn publish(&self, request: PublishRequest<'_>) -> Result<PublishedInvestigation, AdapterError> {
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
    fn the_publisher_guard_stops_a_second_investigation_the_lookup_missed() {
        let fixture = Fixture::new();
        let lagging = LaggingLookup {
            inner: MockSharePoint::open(temp_dir("lag").join("sp.json"), Vec::new).unwrap(),
            lookups: std::sync::atomic::AtomicUsize::new(0),
        };
        let drafts = ConversionFails { inner: &fixture.base.drafts, fail: AtomicBool::new(true) };
        let service = InvestigationService::new(&lagging, &fixture.base.configuration, &drafts);
        let draft = fixture.draft();

        let first = service.complete(&draft.id, 1, &now(), "op").unwrap();
        let second = service.complete(&draft.id, 1, &now(), "op").unwrap();
        assert!(!first.already_existed);
        assert!(second.already_existed);
        assert_eq!(second.investigation.investigation.number, first.investigation.investigation.number);
        assert_eq!(lagging.list().unwrap().len(), 1);
    }

    #[test]
    fn number_conflicts_with_other_drafts_still_retry() {
        // Two different drafts completed back to back get consecutive numbers,
        // i.e. the draft guard does not interfere with number allocation.
        let fixture = Fixture::new();
        let service = fixture.service();
        let a = service.complete(&fixture.draft().id, 1, &now(), "op").unwrap();
        let b = service.complete(&fixture.draft().id, 1, &now(), "op").unwrap();
        assert!(!a.already_existed && !b.already_existed);
        assert_eq!(b.investigation.investigation.number.to_string(), "002-2026");
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
