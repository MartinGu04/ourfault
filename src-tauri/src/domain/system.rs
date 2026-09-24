//! Systems are configuration data managed by administrators, never
//! hard-coded. An activity involves one or more systems, and each system
//! contributes its distribution list.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::validation::{required_text, validate_email, FieldError};

pub const MAX_NAME_CHARS: usize = 80;
pub const MAX_RECIPIENTS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    pub id: String,
    pub name: String,
    /// Inactive systems cannot be chosen for new investigations. Systems are
    /// never deleted, so investigation history stays intact.
    pub active: bool,
    /// May be empty; distribution uses the union of all involved systems.
    pub distribution_list: Vec<String>,
    /// Reserved for future integration metadata (for example the value a
    /// SharePoint choice column uses for this system). Not edited in the UI
    /// yet; preserved on every update.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

/// Administrator input for creating or editing a system. Untrusted until
/// [`SystemInput::apply`] succeeds. Activation is a separate action.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInput {
    /// `None` creates a new system.
    pub id: Option<String>,
    pub name: String,
    pub distribution_list: Vec<String>,
}

impl SystemInput {
    /// Validates the input and applies it to `existing` (or to a new, active
    /// system with id `new_id`).
    pub fn apply(&self, existing: Option<&System>, new_id: String) -> Result<System, Vec<FieldError>> {
        let mut errors = Vec::new();
        let name = required_text(&self.name, MAX_NAME_CHARS).unwrap_or_else(|code| {
            errors.push(FieldError::new("name", code));
            String::new()
        });
        let distribution_list = validate_recipients(&self.distribution_list, &mut errors);
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(match existing {
            Some(system) => System { name, distribution_list, ..system.clone() },
            None => System { id: new_id, name, active: true, distribution_list, metadata: BTreeMap::new() },
        })
    }
}

/// Blank entries are ignored, addresses are lower-cased and de-duplicated.
fn validate_recipients(items: &[String], errors: &mut Vec<FieldError>) -> Vec<String> {
    let mut valid: Vec<String> = Vec::new();
    for item in items.iter().filter(|item| !item.trim().is_empty()) {
        match validate_email(item) {
            Ok(address) if !valid.contains(&address) => valid.push(address),
            Ok(_) => {}
            Err(code) => {
                errors.push(FieldError::new("distributionList", code));
                return Vec::new();
            }
        }
    }
    if valid.len() > MAX_RECIPIENTS {
        errors.push(FieldError::new("distributionList", "too_many"));
    }
    valid
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn system(id: &str, active: bool) -> System {
        System {
            id: id.into(),
            name: format!("מערכת {id}"),
            active,
            distribution_list: vec![format!("{id}@example.com")],
            metadata: BTreeMap::new(),
        }
    }

    fn input() -> SystemInput {
        SystemInput {
            id: None,
            name: "  מערכת דלתא ".into(),
            distribution_list: vec!["Delta-Ops@example.com".into(), "delta-ops@example.com".into(), "".into()],
        }
    }

    #[test]
    fn creates_normalised_active_systems() {
        let system = input().apply(None, "system-9".into()).unwrap();
        assert_eq!(system.id, "system-9");
        assert_eq!(system.name, "מערכת דלתא");
        assert!(system.active);
        assert_eq!(system.distribution_list, vec!["delta-ops@example.com"]);
    }

    #[test]
    fn updates_keep_status_and_integration_metadata() {
        let mut existing = system("alpha", false);
        existing.metadata.insert("sharepointChoice".into(), "ALPHA".into());
        let updated = input().apply(Some(&existing), "unused".into()).unwrap();
        assert_eq!(updated.id, "alpha");
        assert!(!updated.active);
        assert_eq!(updated.metadata["sharepointChoice"], "ALPHA");
    }

    #[test]
    fn reports_invalid_fields() {
        let mut bad = input();
        bad.name = " ".into();
        bad.distribution_list = vec!["not-an-address".into()];
        let errors = bad.apply(None, "x".into()).unwrap_err();
        assert_eq!(
            errors,
            vec![FieldError::new("name", "required"), FieldError::new("distributionList", "invalid_email")]
        );

        let mut many = input();
        many.distribution_list = (0..=MAX_RECIPIENTS).map(|i| format!("user{i}@example.com")).collect();
        assert_eq!(many.apply(None, "x".into()).unwrap_err(), vec![FieldError::new("distributionList", "too_many")]);
    }

    #[test]
    fn an_empty_distribution_list_is_allowed() {
        let mut quiet = input();
        quiet.distribution_list = vec![];
        assert!(quiet.apply(None, "x".into()).unwrap().distribution_list.is_empty());
    }
}
