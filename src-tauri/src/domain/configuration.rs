//! The administrator-managed configuration: systems, stations, technical
//! sections, the distribution e-mail template and the publication target.
//!
//! "Initial setup" and "administration" are the same thing: the setup
//! checklist ([`Configuration::setup_status`]) is computed from the current
//! configuration and stays available after the first run.

use serde::{Deserialize, Serialize};

use super::mail::MailTemplate;
use super::night_window::NightWindow;
use super::sections::{numeric_suffix, SectionDefinition};
use super::station::Station;
use super::system::System;
use super::validation::{required_text, validate_web_url, FieldError, UrlPolicy};

pub const MAX_SYSTEMS: usize = 200;
pub const MAX_STATIONS: usize = 200;
pub const MAX_SECTIONS: usize = 30;
const MAX_LIST_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    pub systems: Vec<System>,
    pub stations: Vec<Station>,
    /// In display order.
    pub sections: Vec<SectionDefinition>,
    pub mail: MailTemplate,
    pub publication: PublicationSettings,
    /// Night hours for the night-activity advisory. Defaults to 20:00 → 06:00
    /// (also for configuration files written before it existed).
    #[serde(default)]
    pub night_window: NightWindow,
}

/// Where completed investigations are created when publishing to SharePoint:
/// one list for all investigations, whatever systems they involve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationSettings {
    pub site_url: String,
    pub list: String,
}

impl PublicationSettings {
    pub fn validated(&self) -> Result<PublicationSettings, Vec<FieldError>> {
        let mut errors = Vec::new();
        let site_url = validate_web_url(&self.site_url, UrlPolicy::HttpsOnly).unwrap_or_else(|code| {
            errors.push(FieldError::new("siteUrl", code));
            String::new()
        });
        let list = required_text(&self.list, MAX_LIST_NAME_CHARS)
            .and_then(|list| {
                if list.chars().any(|c| "/\\:*?\"<>|#%".contains(c)) {
                    Err("invalid_characters")
                } else {
                    Ok(list)
                }
            })
            .unwrap_or_else(|code| {
                errors.push(FieldError::new("list", code));
                String::new()
            });
        if errors.is_empty() {
            Ok(PublicationSettings { site_url, list })
        } else {
            Err(errors)
        }
    }
}

/// One item of the setup checklist. `required` items must be done before
/// investigations can be created; the others are recommendations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupItem {
    pub key: &'static str,
    pub done: bool,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub items: Vec<SetupItem>,
    /// All required items are done.
    pub ready: bool,
}

impl Configuration {
    pub fn system(&self, id: &str) -> Option<&System> {
        self.systems.iter().find(|system| system.id == id)
    }

    pub fn station(&self, id: &str) -> Option<&Station> {
        self.stations.iter().find(|station| station.id == id)
    }

    /// Sections as operators fill them in: active sections, active fields.
    pub fn active_sections(&self) -> Vec<SectionDefinition> {
        self.sections.iter().filter(|section| section.active).map(SectionDefinition::with_active_fields).collect()
    }

    pub fn setup_status(&self) -> SetupStatus {
        let active_systems: Vec<&System> = self.systems.iter().filter(|system| system.active).collect();
        let items = vec![
            SetupItem { key: "systems", done: !active_systems.is_empty(), required: true },
            SetupItem {
                key: "distributionLists",
                done: !active_systems.is_empty() && active_systems.iter().all(|s| !s.distribution_list.is_empty()),
                required: false,
            },
            SetupItem { key: "stations", done: self.stations.iter().any(|s| s.active), required: false },
            SetupItem { key: "sections", done: self.sections.iter().any(|s| s.active), required: false },
            SetupItem { key: "mailTemplate", done: self.mail.validated().is_ok(), required: true },
            SetupItem { key: "publication", done: self.publication.validated().is_ok(), required: true },
        ];
        let ready = items.iter().all(|item| item.done || !item.required);
        SetupStatus { items, ready }
    }
}

/// `{prefix}-{n}` with `n` one higher than any existing numeric suffix.
pub fn next_id<'a>(prefix: &str, existing: impl IntoIterator<Item = &'a str>) -> String {
    let highest = existing.into_iter().filter_map(|id| numeric_suffix(id, prefix)).max().unwrap_or(0);
    format!("{prefix}-{}", highest + 1)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::domain::mail::tests::template;
    use crate::domain::sections::tests::{stations_section, vehicles_section};
    use crate::domain::system::tests::system;

    pub(crate) fn configuration() -> Configuration {
        Configuration {
            systems: vec![system("alpha", true), system("beta", true), system("retired", false)],
            stations: vec![
                Station { id: "st-1".into(), name: "תחנת אורן".into(), active: true },
                Station { id: "st-2".into(), name: "תחנת גפן".into(), active: false },
            ],
            sections: vec![stations_section(), vehicles_section()],
            mail: template(),
            publication: PublicationSettings {
                site_url: "https://sharepoint.example.com/sites/ops".into(),
                list: "Investigations".into(),
            },
            night_window: NightWindow::default(),
        }
    }

    #[test]
    fn active_sections_hide_inactive_sections_and_fields() {
        let mut config = configuration();
        config.sections[0].active = false;
        config.sections[1].fields[1].active = false;
        let sections = config.active_sections();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].id, "vehicles");
        assert_eq!(sections[0].fields.len(), 4);
    }

    #[test]
    fn setup_status_reflects_the_current_configuration() {
        let config = configuration();
        assert!(config.setup_status().ready);

        let mut empty = config.clone();
        empty.systems.iter_mut().for_each(|system| system.active = false);
        empty.stations.clear();
        let status = empty.setup_status();
        assert!(!status.ready);
        let open: Vec<&str> = status.items.iter().filter(|item| !item.done).map(|item| item.key).collect();
        assert_eq!(open, vec!["systems", "distributionLists", "stations"]);
    }

    #[test]
    fn validates_the_publication_target() {
        let bad = PublicationSettings { site_url: "http://sharepoint.example.com".into(), list: "a/b".into() };
        let codes: Vec<(String, &str)> = bad.validated().unwrap_err().into_iter().map(|e| (e.field, e.code)).collect();
        assert_eq!(codes, vec![("siteUrl".into(), "unsupported_scheme"), ("list".into(), "invalid_characters")]);
    }

    #[test]
    fn generates_ids_after_the_highest_existing_one() {
        assert_eq!(next_id("system", ["system-1", "system-7", "custom"]), "system-8");
        assert_eq!(next_id("station", []), "station-1");
    }
}
