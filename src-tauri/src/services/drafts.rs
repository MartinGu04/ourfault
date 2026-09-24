//! Drafts: create, autosave, list, reopen and delete investigations in
//! progress. Drafts never receive an investigation number.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use super::Now;
use crate::adapters::{AdapterError, ConfigurationRepository, DraftRepository};
use crate::domain::draft::{validate_draft_id, Draft, DraftContent, DraftStep, DraftSummary};
use crate::error::AppError;

pub struct DraftService<'a> {
    drafts: &'a dyn DraftRepository,
    configuration: &'a dyn ConfigurationRepository,
}

impl<'a> DraftService<'a> {
    pub fn new(drafts: &'a dyn DraftRepository, configuration: &'a dyn ConfigurationRepository) -> Self {
        Self { drafts, configuration }
    }

    /// Open drafts, most recently edited first.
    pub fn list(&self) -> Result<Vec<DraftSummary>, AppError> {
        let configuration = self.configuration.load()?.value;
        let mut drafts: Vec<Draft> = self.drafts.list()?.into_iter().filter(Draft::is_open).collect();
        drafts.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(drafts
            .into_iter()
            .map(|draft| DraftSummary {
                system_names: draft
                    .content
                    .activity
                    .system_ids
                    .iter()
                    .map(|id| configuration.system(id).map(|s| s.name.clone()).unwrap_or_else(|| id.clone()))
                    .collect(),
                activity_name: draft.content.activity.name.trim().to_owned(),
                row_count: draft.content.rows.len(),
                id: draft.id,
                revision: draft.revision,
                step: draft.step,
                updated_at: draft.updated_at,
                created_by: draft.created_by,
            })
            .collect())
    }

    /// An open draft by id.
    pub fn get(&self, id: &str) -> Result<Draft, AppError> {
        validate_draft_id(id).map_err(|code| AppError::field("draftId", code))?;
        let draft = self.drafts.get(id)?.ok_or(AppError::NotFound)?;
        if !draft.is_open() {
            return Err(AppError::Conflict { code: "draft_converted" });
        }
        Ok(draft)
    }

    pub fn create(&self, content: DraftContent, step: DraftStep, now: &Now, by: &str) -> Result<Draft, AppError> {
        content.check_limits().map_err(|code| AppError::field("draft", code))?;
        let draft = Draft {
            id: generate_id(now),
            revision: 1,
            created_at: now.timestamp.clone(),
            created_by: by.to_owned(),
            updated_at: now.timestamp.clone(),
            step,
            content,
            converted: None,
        };
        self.drafts.save(&draft, None)?;
        Ok(draft)
    }

    /// Autosave: replaces the content of a draft the caller last saw at
    /// `expected_revision`.
    pub fn save(
        &self,
        id: &str,
        expected_revision: u64,
        content: DraftContent,
        step: DraftStep,
        now: &Now,
    ) -> Result<Draft, AppError> {
        content.check_limits().map_err(|code| AppError::field("draft", code))?;
        let current = self.get(id)?;
        if current.revision != expected_revision {
            return Err(AppError::Conflict { code: "changed" });
        }
        let draft =
            Draft { revision: expected_revision + 1, updated_at: now.timestamp.clone(), step, content, ..current };
        self.drafts.save(&draft, Some(expected_revision))?;
        Ok(draft)
    }

    /// Deliberate deletion by the operator.
    pub fn delete(&self, id: &str, expected_revision: u64) -> Result<(), AppError> {
        self.get(id)?;
        self.drafts.delete(id, expected_revision).map_err(|error| match error {
            AdapterError::Conflict => AppError::Conflict { code: "changed" },
            other => other.into(),
        })
    }
}

/// `d-{date}-{64 random bits}`: unique across workstations without
/// coordination, and safe as a file name.
fn generate_id(now: &Now) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    hasher.write(now.timestamp.as_bytes());
    if let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        hasher.write_u128(elapsed.as_nanos());
    }
    format!("d-{}-{:016x}", now.date.format("%Y%m%d"), hasher.finish())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::local_configuration::LocalConfigurationRepository;
    use crate::adapters::local_drafts::LocalDraftRepository;
    use crate::domain::configuration::tests::configuration;
    use crate::services::testing::{now, now_at};

    pub(crate) struct Fixture {
        pub drafts: LocalDraftRepository,
        pub configuration: LocalConfigurationRepository,
    }

    impl Fixture {
        pub(crate) fn new() -> Self {
            let dir = temp_dir("draft-service");
            Self {
                drafts: LocalDraftRepository::open(dir.join("drafts")).unwrap(),
                configuration: LocalConfigurationRepository::open(dir.join("configuration.json"), configuration)
                    .unwrap(),
            }
        }

        pub(crate) fn service(&self) -> DraftService<'_> {
            DraftService::new(&self.drafts, &self.configuration)
        }
    }

    #[test]
    fn autosave_round_trip_continues_where_the_operator_stopped() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let created = service.create(DraftContent::default(), DraftStep::Activity, &now(), "op").unwrap();
        assert!(validate_draft_id(&created.id).is_ok());
        assert_eq!(created.revision, 1);

        let mut content = created.content.clone();
        content.activity.name = "בלט רומני".into();
        content.activity.system_ids = vec!["beta".into()];
        let saved = service.save(&created.id, 1, content.clone(), DraftStep::Chronology, &now_at("11:00")).unwrap();
        assert_eq!(saved.revision, 2);

        // "Restart": a new service over the same storage reopens the draft.
        let reopened = fixture.service().get(&created.id).unwrap();
        assert_eq!(reopened.content, content);
        assert_eq!(reopened.step, DraftStep::Chronology);
        assert_eq!(reopened.created_at, created.created_at);

        let summaries = service.list().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].activity_name, "בלט רומני");
        assert_eq!(summaries[0].system_names, vec!["מערכת beta"]);
    }

    #[test]
    fn autosave_accepts_partial_and_unfinished_times() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let mut content = DraftContent::default();
        content.activity.planned_start = "2026-09-20T08:00".into();
        content.activity.actual_start = "2026-09-20T".into(); // still being typed
        content.activity.actual_end = "2026-09-19T08:00".into(); // before the start, not yet fixed
        let draft = service.create(content.clone(), DraftStep::Activity, &now(), "op").unwrap();
        content.activity.planned_end = "2026-09-20T07:00".into();
        let saved = service.save(&draft.id, 1, content.clone(), DraftStep::Activity, &now()).unwrap();
        assert_eq!(service.get(&saved.id).unwrap().content, content);
    }

    #[test]
    fn stale_autosaves_are_rejected_without_losing_the_stored_draft() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let draft = service.create(DraftContent::default(), DraftStep::Activity, &now(), "op").unwrap();
        let mut first = draft.content.clone();
        first.activity.name = "from workstation A".into();
        service.save(&draft.id, 1, first.clone(), DraftStep::Activity, &now()).unwrap();

        let mut second = draft.content.clone();
        second.activity.name = "from workstation B".into();
        let error = service.save(&draft.id, 1, second, DraftStep::Activity, &now()).unwrap_err();
        assert!(matches!(error, AppError::Conflict { code: "changed" }));
        assert_eq!(service.get(&draft.id).unwrap().content, first);
    }

    #[test]
    fn deletion_is_deliberate_and_revision_checked() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let draft = service.create(DraftContent::default(), DraftStep::Activity, &now(), "op").unwrap();
        assert!(matches!(service.delete(&draft.id, 7), Err(AppError::Conflict { .. })));
        service.delete(&draft.id, 1).unwrap();
        assert!(matches!(service.get(&draft.id), Err(AppError::NotFound)));
        assert!(matches!(service.get("../x"), Err(AppError::Validation { .. })));
    }

    #[test]
    fn lists_most_recent_first_and_hides_converted_drafts() {
        let fixture = Fixture::new();
        let service = fixture.service();
        let older = service.create(DraftContent::default(), DraftStep::Activity, &now_at("08:00"), "op").unwrap();
        let newer = service.create(DraftContent::default(), DraftStep::Activity, &now_at("09:00"), "op").unwrap();
        let ids: Vec<String> = service.list().unwrap().into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec![newer.id.clone(), older.id.clone()]);

        let mut converted = older.clone();
        converted.converted =
            Some(crate::domain::draft::Conversion { number: "056-2026".parse().unwrap(), at: now().timestamp });
        converted.revision = 2;
        fixture.drafts.save(&converted, Some(1)).unwrap();
        assert_eq!(service.list().unwrap().len(), 1);
        assert!(matches!(service.get(&older.id), Err(AppError::Conflict { code: "draft_converted" })));
    }

    #[test]
    fn ids_are_unique() {
        let ids: std::collections::HashSet<String> = (0..500).map(|_| generate_id(&now())).collect();
        assert_eq!(ids.len(), 500);
    }
}
