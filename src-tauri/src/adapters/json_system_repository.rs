//! System configuration stored in a local JSON file.

use std::path::PathBuf;
use std::sync::Mutex;

use super::{json_file, AdapterError, SystemRepository};
use crate::domain::system::System;

pub struct JsonSystemRepository {
    path: PathBuf,
    systems: Mutex<Vec<System>>,
}

impl JsonSystemRepository {
    pub fn open(path: PathBuf, seed: impl FnOnce() -> Vec<System>) -> Result<Self, AdapterError> {
        let systems = json_file::load_or_seed(&path, seed)?;
        Ok(Self { path, systems: Mutex::new(systems) })
    }
}

impl SystemRepository for JsonSystemRepository {
    fn list(&self) -> Result<Vec<System>, AdapterError> {
        let systems = self.systems.lock().map_err(|_| AdapterError::Storage("systems lock poisoned".into()))?;
        Ok(systems.clone())
    }

    fn save(&self, system: System) -> Result<(), AdapterError> {
        let mut systems = self.systems.lock().map_err(|_| AdapterError::Storage("systems lock poisoned".into()))?;
        let mut updated = systems.clone();
        match updated.iter_mut().find(|existing| existing.id == system.id) {
            Some(existing) => *existing = system,
            None => updated.push(system),
        }
        json_file::save(&self.path, &updated)?;
        *systems = updated;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::domain::investigation::tests::system;

    #[test]
    fn inserts_updates_and_persists() {
        let path = temp_dir("systems").join("systems.json");
        let repo = JsonSystemRepository::open(path.clone(), || vec![system("alpha", true)]).unwrap();

        repo.save(system("bravo", true)).unwrap();
        let mut renamed = system("alpha", false);
        renamed.name = "אלפא חדשה".into();
        repo.save(renamed).unwrap();

        let reopened = JsonSystemRepository::open(path, || panic!("must not reseed")).unwrap();
        let systems = reopened.list().unwrap();
        assert_eq!(systems.len(), 2);
        let alpha = reopened.get("alpha").unwrap().unwrap();
        assert_eq!(alpha.name, "אלפא חדשה");
        assert!(!alpha.active);
    }
}
