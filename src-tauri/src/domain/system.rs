//! Systems are configuration data, never hard-coded: every investigation
//! belongs to exactly one, and the system decides the template, the
//! distribution list and where the investigation is stored.

use serde::{Deserialize, Serialize};

use super::validation::{required_text, validate_email, validate_web_url, FieldError, UrlPolicy};

pub const MAX_NAME_CHARS: usize = 80;
pub const MAX_TITLE_CHARS: usize = 120;
pub const MAX_SECTIONS: usize = 20;
pub const MAX_RECIPIENTS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    pub id: String,
    pub name: String,
    /// Inactive systems cannot be chosen for new investigations. Systems are
    /// never deleted so that existing investigation history stays intact.
    pub active: bool,
    pub template: InvestigationTemplate,
    pub distribution_list: Vec<String>,
    pub sharepoint: SharePointDestination,
}

/// The investigation template configured for a system. A copy is stored with
/// every investigation, so later edits never change existing investigations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationTemplate {
    /// Name shown to operators and administrators.
    pub name: String,
    /// Heading of the generated investigation document.
    pub title: String,
    /// Section headings the investigator completes after creation.
    pub sections: Vec<String>,
}

/// Where investigations of a system are created: a SharePoint list whose
/// items are the editable investigation forms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharePointDestination {
    pub site_url: String,
    pub list: String,
}

/// Administrator input for creating or editing a system. Untrusted until
/// [`SystemInput::validate`] succeeds.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInput {
    /// `None` creates a new system.
    pub id: Option<String>,
    pub name: String,
    pub active: bool,
    pub template: InvestigationTemplate,
    pub distribution_list: Vec<String>,
    pub sharepoint: SharePointDestination,
}

impl SystemInput {
    /// Validates and normalises the input into a [`System`] with the given id.
    pub fn validate(&self, id: String) -> Result<System, Vec<FieldError>> {
        let mut errors = Vec::new();
        let mut check = |field: &str, result: Result<String, &'static str>| {
            result.unwrap_or_else(|code| {
                errors.push(FieldError::new(field, code));
                String::new()
            })
        };

        let name = check("name", required_text(&self.name, MAX_NAME_CHARS));
        let template_name = check("templateName", required_text(&self.template.name, MAX_NAME_CHARS));
        let template_title = check("templateTitle", required_text(&self.template.title, MAX_TITLE_CHARS));
        let site_url = check("sharepointSiteUrl", validate_web_url(&self.sharepoint.site_url, UrlPolicy::HttpsOnly));
        let list = check("sharepointList", validate_list_name(&self.sharepoint.list));
        let sections = check_list(&mut errors, "templateSections", &self.template.sections, MAX_SECTIONS, |s| {
            required_text(s, MAX_NAME_CHARS)
        });
        let distribution_list =
            check_list(&mut errors, "distributionList", &self.distribution_list, MAX_RECIPIENTS, validate_email);

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(System {
            id,
            name,
            active: self.active,
            template: InvestigationTemplate { name: template_name, title: template_title, sections },
            distribution_list,
            sharepoint: SharePointDestination { site_url, list },
        })
    }
}

/// Validates a list field: blank entries are ignored, duplicates removed,
/// at least one and at most `max` entries are required.
fn check_list(
    errors: &mut Vec<FieldError>,
    field: &str,
    items: &[String],
    max: usize,
    validate: impl Fn(&str) -> Result<String, &'static str>,
) -> Vec<String> {
    let mut valid: Vec<String> = Vec::new();
    for item in items.iter().filter(|item| !item.trim().is_empty()) {
        match validate(item) {
            Ok(value) if !valid.contains(&value) => valid.push(value),
            Ok(_) => {}
            Err(code) => {
                errors.push(FieldError::new(field, code));
                return Vec::new();
            }
        }
    }
    if valid.is_empty() {
        errors.push(FieldError::new(field, "required"));
    } else if valid.len() > max {
        errors.push(FieldError::new(field, "too_many"));
    }
    valid
}

fn validate_list_name(value: &str) -> Result<String, &'static str> {
    let list = required_text(value, MAX_NAME_CHARS)?;
    if list.chars().any(|c| "/\\:*?\"<>|#%".contains(c)) {
        return Err("invalid_characters");
    }
    Ok(list)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample_input() -> SystemInput {
        SystemInput {
            id: None,
            name: "  מערכת דלתא ".into(),
            active: true,
            template: InvestigationTemplate {
                name: "תבנית בסיסית".into(),
                title: "תחקיר אירוע".into(),
                sections: vec!["רקע".into(), " ".into(), "ממצאים".into()],
            },
            distribution_list: vec!["Delta-Ops@example.com".into(), "delta-ops@example.com".into(), "".into()],
            sharepoint: SharePointDestination {
                site_url: "https://sharepoint.example.com/sites/delta".into(),
                list: "Investigations".into(),
            },
        }
    }

    #[test]
    fn valid_input_is_normalised() {
        let system = sample_input().validate("system-9".into()).unwrap();
        assert_eq!(system.id, "system-9");
        assert_eq!(system.name, "מערכת דלתא");
        assert_eq!(system.template.sections, vec!["רקע", "ממצאים"]);
        assert_eq!(system.distribution_list, vec!["delta-ops@example.com"]);
    }

    #[test]
    fn reports_every_invalid_field() {
        let mut input = sample_input();
        input.name = " ".into();
        input.template.sections = vec![];
        input.distribution_list = vec!["not-an-address".into()];
        input.sharepoint.site_url = "http://sharepoint.example.com".into();
        input.sharepoint.list = "a/b".into();

        let errors = input.validate("x".into()).unwrap_err();
        let fields: Vec<(&str, &str)> = errors.iter().map(|e| (e.field.as_str(), e.code)).collect();
        assert_eq!(
            fields,
            vec![
                ("name", "required"),
                ("sharepointSiteUrl", "unsupported_scheme"),
                ("sharepointList", "invalid_characters"),
                ("templateSections", "required"),
                ("distributionList", "invalid_email"),
            ]
        );
    }

    #[test]
    fn limits_the_number_of_recipients() {
        let mut input = sample_input();
        input.distribution_list = (0..=MAX_RECIPIENTS).map(|i| format!("user{i}@example.com")).collect();
        let errors = input.validate("x".into()).unwrap_err();
        assert_eq!(errors, vec![FieldError::new("distributionList", "too_many")]);
    }
}
