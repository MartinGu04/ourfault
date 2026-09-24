//! Configuration stored in one local JSON file with a revision counter.
//!
//! The file is re-read on every load and checked again before every save,
//! so the revision guard also works between two OurFault processes. A
//! shared implementation (e.g. a SharePoint list item with ETags) keeps the
//! same contract.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::{json_file, AdapterError, ConfigurationRepository, Revisioned};
use crate::domain::configuration::Configuration;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    revision: u64,
    configuration: Configuration,
}

pub struct LocalConfigurationRepository {
    path: PathBuf,
    // Serialises read-check-write within this process.
    lock: Mutex<()>,
}

impl LocalConfigurationRepository {
    /// Opens the store at `path`, seeding it on first run.
    pub fn open(path: PathBuf, seed: impl FnOnce() -> Configuration) -> Result<Self, AdapterError> {
        json_file::load_or_seed(&path, || Stored { revision: 1, configuration: seed() })?;
        Ok(Self { path, lock: Mutex::new(()) })
    }

    fn read(&self) -> Result<Stored, AdapterError> {
        json_file::load(&self.path)?.ok_or_else(|| AdapterError::Storage("configuration file disappeared".into()))
    }
}

impl ConfigurationRepository for LocalConfigurationRepository {
    fn load(&self) -> Result<Revisioned<Configuration>, AdapterError> {
        let stored = self.read()?;
        Ok(Revisioned { value: stored.configuration, revision: stored.revision })
    }

    fn save(&self, configuration: &Configuration, expected_revision: u64) -> Result<u64, AdapterError> {
        let _guard = self.lock.lock().map_err(|_| AdapterError::Storage("configuration lock poisoned".into()))?;
        if self.read()?.revision != expected_revision {
            return Err(AdapterError::Conflict);
        }
        let revision = expected_revision + 1;
        json_file::save(&self.path, &Stored { revision, configuration: configuration.clone() })?;
        Ok(revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::domain::configuration::tests::configuration;

    #[test]
    fn saves_with_revision_checks_and_persists() {
        let path = temp_dir("config").join("configuration.json");
        let repo = LocalConfigurationRepository::open(path.clone(), configuration).unwrap();
        let loaded = repo.load().unwrap();
        assert_eq!(loaded.revision, 1);

        let mut changed = loaded.value.clone();
        changed.stations.clear();
        assert_eq!(repo.save(&changed, 1).unwrap(), 2);
        assert!(matches!(repo.save(&loaded.value, 1), Err(AdapterError::Conflict)), "stale revision");

        let reopened = LocalConfigurationRepository::open(path, || panic!("must not reseed")).unwrap();
        let loaded = reopened.load().unwrap();
        assert_eq!(loaded.revision, 2);
        assert!(loaded.value.stations.is_empty());
    }
}
