//! Configuration: the read-only workspace every mode uses
//! ([`ConfigurationService`]), and the administration operations
//! ([`Administration`]). An `Administration` can only be created with an
//! [`AdminGrant`], which only `AppState::admin` hands out in admin mode.

use serde::Serialize;

use crate::adapters::ConfigurationRepository;
use crate::domain::configuration::{
    next_id, Configuration, PublicationSettings, SetupStatus, MAX_SECTIONS, MAX_STATIONS, MAX_SYSTEMS,
};
use crate::domain::mail::MailTemplate;
use crate::domain::sections::{SectionDefinition, SectionInput};
use crate::domain::station::{Station, StationInput};
use crate::domain::system::{System, SystemInput};
use crate::domain::validation::FieldError;
use crate::error::AppError;
use crate::state::AdminGrant;

/// A selectable reference (system or station). Inactive ones are included
/// so reopened drafts can still show names; only active ones are offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub active: bool,
}

/// What the investigation workflow needs from the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub systems: Vec<Choice>,
    pub stations: Vec<Choice>,
    /// Active sections with their active fields, in order.
    pub sections: Vec<SectionDefinition>,
    pub setup: SetupStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminConfiguration {
    pub configuration: Configuration,
    pub setup: SetupStatus,
}

pub struct ConfigurationService<'a> {
    repository: &'a dyn ConfigurationRepository,
}

impl<'a> ConfigurationService<'a> {
    pub fn new(repository: &'a dyn ConfigurationRepository) -> Self {
        Self { repository }
    }

    pub fn current(&self) -> Result<Configuration, AppError> {
        Ok(self.repository.load()?.value)
    }

    pub fn workspace(&self) -> Result<Workspace, AppError> {
        let configuration = self.current()?;
        let by_name = |mut choices: Vec<Choice>| {
            choices.sort_by(|a, b| b.active.cmp(&a.active).then_with(|| a.name.cmp(&b.name)));
            choices
        };
        Ok(Workspace {
            systems: by_name(
                configuration
                    .systems
                    .iter()
                    .map(|s| Choice { id: s.id.clone(), name: s.name.clone(), active: s.active })
                    .collect(),
            ),
            stations: by_name(
                configuration
                    .stations
                    .iter()
                    .map(|s| Choice { id: s.id.clone(), name: s.name.clone(), active: s.active })
                    .collect(),
            ),
            sections: configuration.active_sections(),
            setup: configuration.setup_status(),
        })
    }
}

/// Administration of the configuration (the same operations serve the
/// initial setup and later changes).
pub struct Administration<'a> {
    repository: &'a dyn ConfigurationRepository,
}

impl<'a> Administration<'a> {
    pub fn new(repository: &'a dyn ConfigurationRepository, _grant: AdminGrant) -> Self {
        Self { repository }
    }

    pub fn current(&self) -> Result<Configuration, AppError> {
        Ok(self.repository.load()?.value)
    }

    pub fn overview(&self) -> Result<AdminConfiguration, AppError> {
        let configuration = self.current()?;
        Ok(AdminConfiguration { setup: configuration.setup_status(), configuration })
    }

    /// Applies `change` to the latest configuration and saves it, guarded by
    /// the revision it was read at.
    fn update<T>(&self, change: impl FnOnce(&mut Configuration) -> Result<T, AppError>) -> Result<T, AppError> {
        let loaded = self.repository.load()?;
        let mut configuration = loaded.value;
        let result = change(&mut configuration)?;
        self.repository.save(&configuration, loaded.revision)?;
        Ok(result)
    }

    pub fn save_system(&self, input: &SystemInput) -> Result<System, AppError> {
        self.update(|configuration| {
            let index = existing_index(&input.id, configuration.systems.iter().map(|s| s.id.as_str()))?;
            let new_id = next_id("system", configuration.systems.iter().map(|s| s.id.as_str()));
            let system = input.apply(index.map(|i| &configuration.systems[i]), new_id).map_err(AppError::validation)?;
            check_unique_name(&system.id, &system.name, configuration.systems.iter().map(|s| (&s.id, &s.name)))?;
            upsert(&mut configuration.systems, index, system.clone(), MAX_SYSTEMS)?;
            Ok(system)
        })
    }

    /// Systems are deactivated, never deleted, so history stays intact.
    pub fn set_system_active(&self, id: &str, active: bool) -> Result<System, AppError> {
        self.update(|configuration| {
            let system = configuration.systems.iter_mut().find(|s| s.id == id).ok_or(AppError::NotFound)?;
            system.active = active;
            Ok(system.clone())
        })
    }

    pub fn save_station(&self, input: &StationInput) -> Result<Station, AppError> {
        self.update(|configuration| {
            let index = existing_index(&input.id, configuration.stations.iter().map(|s| s.id.as_str()))?;
            let new_id = next_id("station", configuration.stations.iter().map(|s| s.id.as_str()));
            let station =
                input.apply(index.map(|i| &configuration.stations[i]), new_id).map_err(AppError::validation)?;
            check_unique_name(&station.id, &station.name, configuration.stations.iter().map(|s| (&s.id, &s.name)))?;
            upsert(&mut configuration.stations, index, station.clone(), MAX_STATIONS)?;
            Ok(station)
        })
    }

    pub fn set_station_active(&self, id: &str, active: bool) -> Result<Station, AppError> {
        self.update(|configuration| {
            let station = configuration.stations.iter_mut().find(|s| s.id == id).ok_or(AppError::NotFound)?;
            station.active = active;
            Ok(station.clone())
        })
    }

    /// Creates a section (appended at the end) or replaces one with its
    /// edited fields.
    pub fn save_section(&self, input: &SectionInput) -> Result<SectionDefinition, AppError> {
        self.update(|configuration| {
            let index = existing_index(&input.id, configuration.sections.iter().map(|s| s.id.as_str()))?;
            let new_id = next_id("section", configuration.sections.iter().map(|s| s.id.as_str()));
            let section =
                input.apply(index.map(|i| &configuration.sections[i]), new_id).map_err(AppError::validation)?;
            check_unique_name(&section.id, &section.name, configuration.sections.iter().map(|s| (&s.id, &s.name)))?;
            upsert(&mut configuration.sections, index, section.clone(), MAX_SECTIONS)?;
            Ok(section)
        })
    }

    pub fn set_section_active(&self, id: &str, active: bool) -> Result<SectionDefinition, AppError> {
        self.update(|configuration| {
            let section = configuration.sections.iter_mut().find(|s| s.id == id).ok_or(AppError::NotFound)?;
            section.active = active;
            Ok(section.clone())
        })
    }

    /// Moves a section one place up (`-1`) or down (`+1`). Moving past
    /// either end changes nothing. Returns the new order of section ids.
    pub fn move_section(&self, id: &str, offset: i32) -> Result<Vec<String>, AppError> {
        self.update(|configuration| {
            let from = configuration.sections.iter().position(|s| s.id == id).ok_or(AppError::NotFound)?;
            let to = if offset < 0 { from.checked_sub(1) } else { Some(from + 1) };
            if let Some(to) = to.filter(|to| *to < configuration.sections.len()) {
                configuration.sections.swap(from, to);
            }
            Ok(configuration.sections.iter().map(|s| s.id.clone()).collect())
        })
    }

    pub fn save_mail_template(&self, input: &MailTemplate) -> Result<MailTemplate, AppError> {
        let template = input.validated().map_err(AppError::validation)?;
        self.update(|configuration| {
            configuration.mail = template.clone();
            Ok(template)
        })
    }

    pub fn save_publication(&self, input: &PublicationSettings) -> Result<PublicationSettings, AppError> {
        let settings = input.validated().map_err(AppError::validation)?;
        self.update(|configuration| {
            configuration.publication = settings.clone();
            Ok(settings)
        })
    }
}

/// Index of the item named by `id` (`None` for a new item). An unknown id is
/// an error rather than silently creating something new.
fn existing_index<'s>(id: &Option<String>, ids: impl Iterator<Item = &'s str>) -> Result<Option<usize>, AppError> {
    match id {
        None => Ok(None),
        Some(id) => ids.into_iter().position(|existing| existing == id).map(Some).ok_or(AppError::NotFound),
    }
}

fn check_unique_name<'s>(
    id: &str,
    name: &str,
    existing: impl Iterator<Item = (&'s String, &'s String)>,
) -> Result<(), AppError> {
    let taken =
        existing.into_iter().any(|(other_id, other)| other_id != id && other.to_lowercase() == name.to_lowercase());
    if taken {
        Err(AppError::validation(vec![FieldError::new("name", "duplicate_name")]))
    } else {
        Ok(())
    }
}

fn upsert<T>(items: &mut Vec<T>, index: Option<usize>, item: T, max: usize) -> Result<(), AppError> {
    match index {
        Some(index) => items[index] = item,
        None if items.len() >= max => return Err(AppError::field("name", "too_many")),
        None => items.push(item),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::local_configuration::LocalConfigurationRepository;
    use crate::domain::configuration::tests::configuration;
    use crate::domain::sections::{FieldInput, FieldKind, SectionMode};

    fn admin(repository: &LocalConfigurationRepository) -> Administration<'_> {
        Administration::new(repository, AdminGrant::for_tests())
    }

    fn repository() -> LocalConfigurationRepository {
        LocalConfigurationRepository::open(temp_dir("config").join("configuration.json"), configuration).unwrap()
    }

    #[test]
    fn workspace_offers_active_sections_and_labels_every_choice() {
        let repo = repository();
        let workspace = ConfigurationService::new(&repo).workspace().unwrap();
        let systems: Vec<(&str, bool)> = workspace.systems.iter().map(|s| (s.id.as_str(), s.active)).collect();
        assert_eq!(systems, vec![("alpha", true), ("beta", true), ("retired", false)]);
        assert_eq!(workspace.sections.len(), 2);
        assert!(workspace.setup.ready);
    }

    #[test]
    fn manages_systems_without_deleting_them() {
        let repo = repository();
        let service = admin(&repo);
        let input =
            SystemInput {
                id: None, name: "מערכת דלתא".into(), distribution_list: vec!["delta@example.com".into()]
            };
        let created = service.save_system(&input).unwrap();
        assert_eq!(created.id, "system-1");

        let duplicate = SystemInput { name: "מערכת ALPHA".into(), ..input.clone() };
        assert!(
            matches!(service.save_system(&duplicate), Err(AppError::Validation { ref errors }) if errors[0].code == "duplicate_name")
        );
        let unknown = SystemInput { id: Some("system-99".into()), ..input };
        assert!(matches!(service.save_system(&unknown), Err(AppError::NotFound)));

        service.set_system_active("alpha", false).unwrap();
        let systems = service.current().unwrap().systems;
        assert_eq!(systems.len(), 4, "deactivated, not deleted");
        assert!(!systems.iter().find(|s| s.id == "alpha").unwrap().active);
    }

    #[test]
    fn manages_stations() {
        let repo = repository();
        let service = admin(&repo);
        let created = service.save_station(&StationInput { id: None, name: "תחנת ברוש".into() }).unwrap();
        assert_eq!(created.id, "station-1");
        service.save_station(&StationInput { id: Some("st-1".into()), name: "תחנת אורן החדשה".into() }).unwrap();
        service.set_station_active("station-1", false).unwrap();
        let stations = service.current().unwrap().stations;
        assert_eq!(stations[0].name, "תחנת אורן החדשה");
        assert!(!stations[2].active);
    }

    #[test]
    fn sections_are_created_reordered_and_deactivated() {
        let repo = repository();
        let service = admin(&repo);
        let input = SectionInput {
            id: None,
            name: "תנאים".into(),
            mode: SectionMode::Single,
            fields: vec![FieldInput {
                id: None,
                label: "משתתפים".into(),
                kind: FieldKind::Number,
                required: true,
                active: true,
                options: vec![],
            }],
        };
        let created = service.save_section(&input).unwrap();
        assert_eq!(created.id, "section-1");

        assert_eq!(service.move_section("section-1", -1).unwrap(), vec!["links", "section-1", "vehicles"]);
        assert_eq!(service.move_section("section-1", -1).unwrap(), vec!["section-1", "links", "vehicles"]);
        assert_eq!(
            service.move_section("section-1", -1).unwrap(),
            vec!["section-1", "links", "vehicles"],
            "stays first"
        );
        assert_eq!(service.move_section("vehicles", 1).unwrap(), vec!["section-1", "links", "vehicles"], "stays last");

        service.set_section_active("links", false).unwrap();
        let active: Vec<String> =
            ConfigurationService::new(&repo).workspace().unwrap().sections.into_iter().map(|s| s.id).collect();
        assert_eq!(active, vec!["section-1", "vehicles"]);
    }

    #[test]
    fn mail_template_and_publication_are_validated() {
        let repo = repository();
        let service = admin(&repo);
        let mut template = service.current().unwrap().mail;
        template.body = "{{UNKNOWN}}".into();
        assert!(matches!(service.save_mail_template(&template), Err(AppError::Validation { .. })));
        template.body = "תחקיר {{INVESTIGATION_NUMBER}}: {{INVESTIGATION_LINK}}".into();
        service.save_mail_template(&template).unwrap();
        assert_eq!(service.current().unwrap().mail.body, template.body);

        let publication =
            PublicationSettings { site_url: "https://sharepoint.example.com/sites/new".into(), list: "Reports".into() };
        service.save_publication(&publication).unwrap();
        assert_eq!(service.current().unwrap().publication, publication);
    }
}
