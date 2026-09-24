//! Drafts as one JSON file per draft in a local folder.
//!
//! One file per draft keeps writes small and independent, which is also the
//! layout a future shared-folder implementation would use. Every write
//! checks the draft's revision first.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use super::{json_file, AdapterError, DraftRepository};
use crate::domain::draft::{validate_draft_id, Draft};

pub struct LocalDraftRepository {
    folder: PathBuf,
    // Serialises read-check-write within this process.
    lock: Mutex<()>,
}

impl LocalDraftRepository {
    pub fn open(folder: PathBuf) -> Result<Self, AdapterError> {
        fs::create_dir_all(&folder)?;
        Ok(Self { folder, lock: Mutex::new(()) })
    }

    /// Seeds demo drafts when the folder is new.
    pub fn open_with_seed(folder: PathBuf, seed: impl FnOnce() -> Vec<Draft>) -> Result<Self, AdapterError> {
        let fresh = !folder.exists();
        let repository = Self::open(folder)?;
        if fresh {
            for draft in seed() {
                repository.save(&draft, None)?;
            }
        }
        Ok(repository)
    }

    fn path(&self, id: &str) -> Result<PathBuf, AdapterError> {
        // Ids come from the webview: never let one leave the folder.
        validate_draft_id(id).map_err(|code| AdapterError::Storage(format!("{code}: {id:?}")))?;
        Ok(self.folder.join(format!("{id}.json")))
    }

    fn guard(&self) -> Result<std::sync::MutexGuard<'_, ()>, AdapterError> {
        self.lock.lock().map_err(|_| AdapterError::Storage("draft lock poisoned".into()))
    }
}

impl DraftRepository for LocalDraftRepository {
    fn list(&self) -> Result<Vec<Draft>, AdapterError> {
        let mut drafts = Vec::new();
        for entry in fs::read_dir(&self.folder)? {
            let path = entry?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            // One unreadable draft must not hide all the others.
            match json_file::load::<Draft>(&path) {
                Ok(Some(draft)) => drafts.push(draft),
                Ok(None) => {}
                Err(error) => crate::log_internal("reading draft", &error),
            }
        }
        Ok(drafts)
    }

    fn get(&self, id: &str) -> Result<Option<Draft>, AdapterError> {
        json_file::load(&self.path(id)?)
    }

    fn save(&self, draft: &Draft, expected_revision: Option<u64>) -> Result<(), AdapterError> {
        let path = self.path(&draft.id)?;
        let _guard = self.guard()?;
        let current = json_file::load::<Draft>(&path)?;
        match (current, expected_revision) {
            (None, None) => {}
            (Some(stored), Some(expected)) if stored.revision == expected => {}
            _ => return Err(AdapterError::Conflict),
        }
        json_file::save(&path, draft)
    }

    fn delete(&self, id: &str, expected_revision: u64) -> Result<(), AdapterError> {
        let path = self.path(id)?;
        let _guard = self.guard()?;
        match json_file::load::<Draft>(&path)? {
            Some(stored) if stored.revision == expected_revision => Ok(fs::remove_file(path)?),
            _ => Err(AdapterError::Conflict),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::domain::draft::{DraftContent, DraftStep};

    pub(crate) fn draft(id: &str, revision: u64) -> Draft {
        Draft {
            id: id.into(),
            revision,
            created_at: "2026-09-23T10:00:00+03:00".into(),
            created_by: "op".into(),
            updated_at: "2026-09-23T10:00:00+03:00".into(),
            step: DraftStep::Activity,
            content: DraftContent::default(),
            converted: None,
        }
    }

    #[test]
    fn creates_updates_and_deletes_with_revision_checks() {
        let repo = LocalDraftRepository::open(temp_dir("drafts")).unwrap();
        repo.save(&draft("d-1", 1), None).unwrap();
        assert!(matches!(repo.save(&draft("d-1", 1), None), Err(AdapterError::Conflict)), "ids are unique");

        let mut edited = draft("d-1", 2);
        edited.content.activity.name = "בלט רומני".into();
        repo.save(&edited, Some(1)).unwrap();
        assert!(matches!(repo.save(&draft("d-1", 2), Some(1)), Err(AdapterError::Conflict)), "stale write");
        assert_eq!(repo.get("d-1").unwrap().unwrap().content.activity.name, "בלט רומני");

        assert!(matches!(repo.delete("d-1", 1), Err(AdapterError::Conflict)));
        repo.delete("d-1", 2).unwrap();
        assert_eq!(repo.get("d-1").unwrap(), None);
    }

    #[test]
    fn lists_readable_drafts_and_skips_broken_files() {
        let folder = temp_dir("drafts");
        let repo = LocalDraftRepository::open(folder.clone()).unwrap();
        repo.save(&draft("d-1", 1), None).unwrap();
        repo.save(&draft("d-2", 1), None).unwrap();
        fs::write(folder.join("d-3.json"), "garbage").unwrap();
        fs::write(folder.join("notes.txt"), "ignored").unwrap();
        let mut ids: Vec<String> = repo.list().unwrap().into_iter().map(|d| d.id).collect();
        ids.sort();
        assert_eq!(ids, vec!["d-1", "d-2"]);
    }

    #[test]
    fn refuses_ids_that_would_escape_the_folder() {
        let repo = LocalDraftRepository::open(temp_dir("drafts")).unwrap();
        assert!(repo.get("../configuration").is_err());
        assert!(repo.save(&draft("..\\x", 1), None).is_err());
    }

    #[test]
    fn seeds_only_a_new_folder() {
        let folder = temp_dir("drafts").join("drafts");
        LocalDraftRepository::open_with_seed(folder.clone(), || vec![draft("d-seed", 1)]).unwrap();
        let repo = LocalDraftRepository::open_with_seed(folder, || panic!("must not reseed")).unwrap();
        assert_eq!(repo.list().unwrap().len(), 1);
    }
}
