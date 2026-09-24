use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::investigation_number::InvestigationNumber;
use super::log_rows::{validate_rows, LogRow};
use super::system::{InvestigationTemplate, System};
use super::validation::{validate_preliminary_check_url, FieldError};

/// What the operator prepared in the wizard. Untrusted input from the webview:
/// the rows are whatever the operator pasted and reviewed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationDraft {
    pub system_id: String,
    pub preliminary_check_url: String,
    pub rows: Vec<LogRow>,
}

/// Snapshot of the system at creation time, so renaming a system later does
/// not rewrite history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemRef {
    pub id: String,
    pub name: String,
}

/// The fields of an investigation as they are created in SharePoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Investigation {
    pub number: InvestigationNumber,
    pub date: NaiveDate,
    pub system: SystemRef,
    pub template: InvestigationTemplate,
    pub preliminary_check_url: String,
    pub rows: Vec<LogRow>,
}

impl Investigation {
    /// Builds the investigation from the operator's input, applying every
    /// business rule and reporting all violations at once. `system` is `None`
    /// when no (known) system was chosen. `number` is the candidate number;
    /// the final number may change at creation time (see
    /// `services::investigations`).
    pub fn build(
        number: InvestigationNumber,
        date: NaiveDate,
        system: Option<&System>,
        rows: &[LogRow],
        preliminary_check_url: &str,
    ) -> Result<Self, Vec<FieldError>> {
        let mut errors = Vec::new();

        match system {
            None => errors.push(FieldError::new("systemId", "required")),
            Some(system) if !system.active => errors.push(FieldError::new("systemId", "system_inactive")),
            Some(_) => {}
        }

        let url = validate_preliminary_check_url(preliminary_check_url).unwrap_or_else(|code| {
            errors.push(FieldError::new("preliminaryCheckUrl", code));
            String::new()
        });

        let rows = validate_rows(rows).unwrap_or_else(|code| {
            errors.push(FieldError::new("rows", code));
            Vec::new()
        });

        let system = match system {
            Some(system) if errors.is_empty() => system,
            _ => return Err(errors),
        };

        Ok(Self {
            number,
            date,
            system: SystemRef { id: system.id.clone(), name: system.name.clone() },
            template: system.template.clone(),
            preliminary_check_url: url,
            rows,
        })
    }

    pub fn with_number(mut self, number: InvestigationNumber) -> Self {
        self.number = number;
        self
    }
}

/// An investigation after it has been saved by the SharePoint adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredInvestigation {
    #[serde(flatten)]
    pub investigation: Investigation,
    /// RFC 3339 local timestamp.
    pub created_at: String,
    pub created_by: String,
    /// SharePoint list item id.
    pub item_id: u32,
    /// Address of the editable SharePoint item (fictional for the mock).
    pub url: String,
}

/// Compact form for lists on the home screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationSummary {
    pub number: InvestigationNumber,
    pub date: NaiveDate,
    pub system_name: String,
    pub row_count: usize,
}

impl From<&StoredInvestigation> for InvestigationSummary {
    fn from(stored: &StoredInvestigation) -> Self {
        let inv = &stored.investigation;
        Self { number: inv.number, date: inv.date, system_name: inv.system.name.clone(), row_count: inv.rows.len() }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::domain::system::SharePointDestination;

    pub(crate) fn system(id: &str, active: bool) -> System {
        System {
            id: id.into(),
            name: format!("מערכת {id}"),
            active,
            template: InvestigationTemplate {
                name: format!("תבנית {id}"),
                title: "תחקיר אירוע".into(),
                sections: vec!["רקע".into(), "ממצאים".into()],
            },
            distribution_list: vec![format!("{id}@example.com")],
            sharepoint: SharePointDestination {
                site_url: format!("https://sharepoint.example.com/sites/{id}"),
                list: "Investigations".into(),
            },
        }
    }

    pub(crate) fn rows() -> Vec<LogRow> {
        let row = |time: &str, description: &str| LogRow {
            time: time.into(),
            from: "מוקד".into(),
            to: "עמדה 3".into(),
            description: description.into(),
        };
        vec![row("08:00", "first"), row("08:10", "second")]
    }

    fn number() -> InvestigationNumber {
        InvestigationNumber::new(56, 2026).unwrap()
    }

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 23).unwrap()
    }

    const URL: &str = "https://checks.example.com/runs/1";

    #[test]
    fn builds_from_the_reviewed_rows_in_the_given_order() {
        let mut reviewed = rows();
        reviewed.reverse();
        let inv = Investigation::build(number(), date(), Some(&system("alpha", true)), &reviewed, URL).unwrap();
        let descriptions: Vec<&str> = inv.rows.iter().map(|r| r.description.as_str()).collect();
        assert_eq!(descriptions, vec!["second", "first"], "the operator's order is kept");
        assert_eq!(inv.number.to_string(), "056-2026");
        assert_eq!(inv.preliminary_check_url, URL);
    }

    #[test]
    fn uses_the_template_of_the_selected_system() {
        let alpha = Investigation::build(number(), date(), Some(&system("alpha", true)), &rows(), URL).unwrap();
        let bravo = Investigation::build(number(), date(), Some(&system("bravo", true)), &rows(), URL).unwrap();
        assert_eq!(alpha.template.name, "תבנית alpha");
        assert_eq!(bravo.template.name, "תבנית bravo");
        assert_eq!(bravo.system, SystemRef { id: "bravo".into(), name: "מערכת bravo".into() });
    }

    #[test]
    fn rejects_inactive_systems() {
        let errors = Investigation::build(number(), date(), Some(&system("old", false)), &rows(), URL).unwrap_err();
        assert_eq!(errors, vec![FieldError::new("systemId", "system_inactive")]);
    }

    #[test]
    fn reports_all_missing_inputs_together() {
        let errors = Investigation::build(number(), date(), None, &[], "ftp://x.example.com").unwrap_err();
        assert_eq!(
            errors,
            vec![
                FieldError::new("systemId", "required"),
                FieldError::new("preliminaryCheckUrl", "unsupported_scheme"),
                FieldError::new("rows", "no_rows"),
            ]
        );
    }

    #[test]
    fn serialises_dates_and_numbers_as_strings() {
        let inv = Investigation::build(number(), date(), Some(&system("alpha", true)), &rows(), URL).unwrap();
        let json = serde_json::to_value(&inv).unwrap();
        assert_eq!(json["date"], "2026-09-23");
        assert_eq!(json["number"], "056-2026");
    }
}
