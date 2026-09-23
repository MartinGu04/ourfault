use crate::adapters::SystemRepository;
use crate::domain::system::{System, SystemInput};
use crate::domain::validation::FieldError;
use crate::error::AppError;

/// System administration. There is intentionally no delete: systems are
/// deactivated so investigation history keeps pointing at a real system.
pub struct SystemService<'a> {
    repository: &'a dyn SystemRepository,
}

impl<'a> SystemService<'a> {
    pub fn new(repository: &'a dyn SystemRepository) -> Self {
        Self { repository }
    }

    /// All systems: active first, then by name.
    pub fn list(&self) -> Result<Vec<System>, AppError> {
        let mut systems = self.repository.list()?;
        systems.sort_by(|a, b| b.active.cmp(&a.active).then_with(|| a.name.cmp(&b.name)));
        Ok(systems)
    }

    /// Systems that may be chosen for a new investigation.
    pub fn active(&self) -> Result<Vec<System>, AppError> {
        Ok(self.list()?.into_iter().filter(|system| system.active).collect())
    }

    /// Creates a system (`input.id == None`) or updates an existing one.
    pub fn save(&self, input: &SystemInput) -> Result<System, AppError> {
        let existing = self.repository.list()?;
        let id = match &input.id {
            Some(id) if existing.iter().any(|system| &system.id == id) => id.clone(),
            Some(_) => return Err(AppError::NotFound),
            None => next_id(&existing),
        };
        let system = input.validate(id).map_err(AppError::validation)?;

        let duplicate_name = existing
            .iter()
            .any(|other| other.id != system.id && other.name.to_lowercase() == system.name.to_lowercase());
        if duplicate_name {
            return Err(AppError::validation(vec![FieldError::new("name", "duplicate_name")]));
        }

        self.repository.save(system.clone())?;
        Ok(system)
    }

    pub fn set_active(&self, id: &str, active: bool) -> Result<System, AppError> {
        let mut system = self.repository.get(id)?.ok_or(AppError::NotFound)?;
        system.active = active;
        self.repository.save(system.clone())?;
        Ok(system)
    }
}

/// `system-{n}` with `n` one higher than any existing numeric suffix.
fn next_id(existing: &[System]) -> String {
    let highest = existing
        .iter()
        .filter_map(|system| system.id.strip_prefix("system-").and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);
    format!("system-{}", highest + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::json_system_repository::JsonSystemRepository;
    use crate::domain::investigation::tests::system;
    use crate::domain::system::tests::sample_input;

    fn repository() -> JsonSystemRepository {
        JsonSystemRepository::open(temp_dir("systems").join("systems.json"), || {
            vec![system("system-1", true), system("system-2", false)]
        })
        .unwrap()
    }

    #[test]
    fn creates_systems_with_new_ids() {
        let repo = repository();
        let service = SystemService::new(&repo);
        let created = service.save(&sample_input()).unwrap();
        assert_eq!(created.id, "system-3");
        assert_eq!(repo.list().unwrap().len(), 3);
    }

    #[test]
    fn updates_existing_systems() {
        let repo = repository();
        let service = SystemService::new(&repo);
        let mut input = sample_input();
        input.id = Some("system-1".into());
        input.template.name = "תבנית מעודכנת".into();
        service.save(&input).unwrap();
        assert_eq!(repo.get("system-1").unwrap().unwrap().template.name, "תבנית מעודכנת");

        input.id = Some("system-99".into());
        assert!(matches!(service.save(&input), Err(AppError::NotFound)));
    }

    #[test]
    fn rejects_duplicate_names() {
        let repo = repository();
        let service = SystemService::new(&repo);
        let mut input = sample_input();
        input.name = "מערכת system-2".into();
        let error = service.save(&input).unwrap_err();
        assert!(matches!(error, AppError::Validation { ref errors } if errors[0].code == "duplicate_name"));
    }

    #[test]
    fn activation_controls_availability_for_new_investigations() {
        let repo = repository();
        let service = SystemService::new(&repo);
        let ids = |systems: Vec<System>| systems.into_iter().map(|s| s.id).collect::<Vec<_>>();
        assert_eq!(ids(service.active().unwrap()), vec!["system-1"]);

        service.set_active("system-2", true).unwrap();
        service.set_active("system-1", false).unwrap();
        assert_eq!(ids(service.active().unwrap()), vec!["system-2"]);
        assert_eq!(ids(service.list().unwrap()), vec!["system-2", "system-1"], "deactivated, not deleted");
    }
}
