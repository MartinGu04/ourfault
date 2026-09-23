use super::Now;
use crate::adapters::{AdapterError, SharePointAdapter, SystemRepository};
use crate::domain::investigation::{Investigation, InvestigationDraft, InvestigationSummary, StoredInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::operations_log::OperationsLog;
use crate::domain::system::System;
use crate::error::AppError;

/// How often creation retries with a fresh number after a conflict.
const MAX_ALLOCATION_ATTEMPTS: usize = 5;

pub struct InvestigationService<'a> {
    sharepoint: &'a dyn SharePointAdapter,
    systems: &'a dyn SystemRepository,
}

impl<'a> InvestigationService<'a> {
    pub fn new(sharepoint: &'a dyn SharePointAdapter, systems: &'a dyn SystemRepository) -> Self {
        Self { sharepoint, systems }
    }

    /// The number the next investigation is expected to get. Informational
    /// only: the number is reserved atomically by [`Self::create`].
    pub fn next_number(&self, now: &Now) -> Result<InvestigationNumber, AppError> {
        let year = now.year()?;
        let highest = self.sharepoint.highest_number_in_year(year)?;
        InvestigationNumber::next_after(highest, year).map_err(|e| AppError::internal("next number", &e))
    }

    /// Validates the draft and returns the investigation as it would be
    /// created now.
    pub fn preview(
        &self,
        draft: &InvestigationDraft,
        log: &OperationsLog,
        now: &Now,
    ) -> Result<Investigation, AppError> {
        self.prepare(draft, log, now).map(|(investigation, _)| investigation)
    }

    /// Creates the investigation through the SharePoint adapter.
    ///
    /// Numbering is optimistic: take the next free number, ask the adapter to
    /// create-if-absent, and on a conflict (another workstation was faster)
    /// retry with a fresh number. Uniqueness is guaranteed by the adapter,
    /// not by the UI or by this process.
    pub fn create(
        &self,
        draft: &InvestigationDraft,
        log: &OperationsLog,
        now: &Now,
        created_by: &str,
    ) -> Result<StoredInvestigation, AppError> {
        let (investigation, system) = self.prepare(draft, log, now)?;
        let mut candidate = investigation.number;
        for _ in 0..MAX_ALLOCATION_ATTEMPTS {
            let attempt = investigation.clone().with_number(candidate);
            match self.sharepoint.create_investigation(attempt, &system.sharepoint, created_by, &now.timestamp) {
                Ok(stored) => return Ok(stored),
                Err(AdapterError::NumberTaken(taken)) => {
                    // Prefer the backend's view; never retry the same number.
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
        Err(AppError::internal("create investigation", &"could not allocate a free investigation number"))
    }

    /// Most recent investigations first.
    pub fn recent(&self, limit: usize) -> Result<Vec<InvestigationSummary>, AppError> {
        let mut all = self.sharepoint.list_investigations()?;
        all.sort_by(|a, b| b.investigation.number.cmp(&a.investigation.number));
        Ok(all.iter().take(limit).map(InvestigationSummary::from).collect())
    }

    /// Looks up an investigation by user-entered number (`056-2026`, `56-2026`
    /// or `56` for the current year).
    pub fn find(&self, query: &str, now: &Now) -> Result<StoredInvestigation, AppError> {
        let number = InvestigationNumber::parse_lenient(query, now.year()?)
            .map_err(|_| AppError::field("query", "invalid_number"))?;
        self.sharepoint.find_investigation(number)?.ok_or(AppError::NotFound)
    }

    fn prepare(
        &self,
        draft: &InvestigationDraft,
        log: &OperationsLog,
        now: &Now,
    ) -> Result<(Investigation, System), AppError> {
        let system = self.systems.get(&draft.system_id)?;
        let number = self.next_number(now)?;
        let investigation = Investigation::build(
            number,
            now.date,
            system.as_ref(),
            log,
            &draft.selected_row_ids,
            &draft.preliminary_check_url,
        )
        .map_err(AppError::validation)?;
        // `build` only succeeds with a system.
        let system = system.ok_or(AppError::Internal)?;
        Ok((investigation, system))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::json_system_repository::JsonSystemRepository;
    use crate::adapters::mock_sharepoint::MockSharePoint;
    use crate::domain::investigation::tests::{log, system};
    use crate::domain::system::SharePointDestination;
    use crate::services::testing::now;

    struct Fixture {
        sharepoint: MockSharePoint,
        systems: JsonSystemRepository,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = temp_dir("service");
            Self {
                sharepoint: MockSharePoint::open(dir.join("sp.json"), Vec::new).unwrap(),
                systems: JsonSystemRepository::open(dir.join("systems.json"), || {
                    vec![system("alpha", true), system("bravo", true), system("retired", false)]
                })
                .unwrap(),
            }
        }

        fn service(&self) -> InvestigationService<'_> {
            InvestigationService::new(&self.sharepoint, &self.systems)
        }
    }

    fn draft(system_id: &str) -> InvestigationDraft {
        InvestigationDraft {
            import_id: 1,
            selected_row_ids: vec![2, 4],
            system_id: system_id.into(),
            preliminary_check_url: "https://checks.example.com/runs/77".into(),
        }
    }

    #[test]
    fn preview_proposes_the_next_number_without_reserving_it() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let preview = service.preview(&draft("alpha"), &log(), &now()).unwrap();
        assert_eq!(preview.number.to_string(), "001-2026");
        assert_eq!(preview.template.name, "תבנית alpha");
        assert!(fixture.sharepoint.list_investigations().unwrap().is_empty());
    }

    #[test]
    fn creation_allocates_consecutive_numbers() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let first = service.create(&draft("alpha"), &log(), &now(), "operator").unwrap();
        let second = service.create(&draft("bravo"), &log(), &now(), "operator").unwrap();
        assert_eq!(first.investigation.number.to_string(), "001-2026");
        assert_eq!(second.investigation.number.to_string(), "002-2026");
        assert_eq!(second.location, "https://sharepoint.example.com/sites/bravo/תחקירים/002-2026");
        assert_eq!(service.recent(10).unwrap()[0].number, second.investigation.number);
    }

    #[test]
    fn inactive_and_unknown_systems_are_rejected() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let error = service.create(&draft("retired"), &log(), &now(), "operator").unwrap_err();
        assert!(matches!(error, AppError::Validation { ref errors } if errors[0].code == "system_inactive"));
        let error = service.preview(&draft("missing"), &log(), &now()).unwrap_err();
        assert!(matches!(error, AppError::Validation { ref errors } if errors[0].field == "systemId"));
    }

    /// Simulates another workstation creating an investigation between our
    /// number lookup and our insert.
    struct RacingSharePoint {
        inner: MockSharePoint,
        raced: AtomicBool,
    }

    impl SharePointAdapter for RacingSharePoint {
        fn list_investigations(&self) -> Result<Vec<StoredInvestigation>, AdapterError> {
            self.inner.list_investigations()
        }
        fn find_investigation(&self, n: InvestigationNumber) -> Result<Option<StoredInvestigation>, AdapterError> {
            self.inner.find_investigation(n)
        }
        fn highest_number_in_year(&self, year: u16) -> Result<Option<InvestigationNumber>, AdapterError> {
            self.inner.highest_number_in_year(year)
        }
        fn create_investigation(
            &self,
            investigation: Investigation,
            destination: &SharePointDestination,
            created_by: &str,
            created_at: &str,
        ) -> Result<StoredInvestigation, AdapterError> {
            if !self.raced.swap(true, Ordering::SeqCst) {
                let competitor = investigation.clone();
                self.inner.create_investigation(competitor, destination, "other workstation", created_at)?;
            }
            self.inner.create_investigation(investigation, destination, created_by, created_at)
        }
    }

    #[test]
    fn concurrent_creation_retries_with_the_next_free_number() {
        let fixture = Fixture::new();
        let racing = RacingSharePoint {
            inner: MockSharePoint::open(temp_dir("race").join("sp.json"), Vec::new).unwrap(),
            raced: AtomicBool::new(false),
        };
        let service = InvestigationService::new(&racing, &fixture.systems);

        let preview = service.preview(&draft("alpha"), &log(), &now()).unwrap();
        let created = service.create(&draft("alpha"), &log(), &now(), "operator").unwrap();

        assert_eq!(preview.number.to_string(), "001-2026");
        assert_eq!(created.investigation.number.to_string(), "002-2026");
        let owners: Vec<String> = racing.list_investigations().unwrap().into_iter().map(|s| s.created_by).collect();
        assert_eq!(owners, vec!["other workstation", "operator"]);
    }

    #[test]
    fn finds_investigations_by_lenient_number() {
        let fixture = Fixture::new();
        let service = fixture.service();
        service.create(&draft("alpha"), &log(), &now(), "operator").unwrap();
        assert_eq!(service.find("1", &now()).unwrap().investigation.number.to_string(), "001-2026");
        assert_eq!(service.find(" 001-2026 ", &now()).unwrap().created_by, "operator");
        assert!(matches!(service.find("2-2026", &now()), Err(AppError::NotFound)));
        assert!(matches!(service.find("abc", &now()), Err(AppError::Validation { .. })));
    }
}
