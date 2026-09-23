use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::investigation_number::InvestigationNumber;
use super::operations_log::OperationsLog;
use super::system::{InvestigationTemplate, System};
use super::validation::{validate_preliminary_check_url, FieldError};

/// Upper bound on rows in a single investigation; keeps documents readable
/// and bounds the size of what we store and distribute.
pub const MAX_ROWS_PER_INVESTIGATION: usize = 500;

/// What the operator chose in the wizard. Untrusted input from the webview.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationDraft {
    pub import_id: u64,
    pub selected_row_ids: Vec<u32>,
    pub system_id: String,
    pub preliminary_check_url: String,
}

/// Snapshot of the system at creation time, so renaming a system later does
/// not rewrite history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemRef {
    pub id: String,
    pub name: String,
}

/// A row as it appears in the investigation. The event type is intentionally
/// not included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationRow {
    pub time: String,
    pub from: String,
    pub to: String,
    pub description: String,
}

/// The content of an investigation document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Investigation {
    pub number: InvestigationNumber,
    pub date: NaiveDate,
    pub system: SystemRef,
    pub template: InvestigationTemplate,
    pub preliminary_check_url: String,
    pub rows: Vec<InvestigationRow>,
    pub source_file_name: String,
}

impl Investigation {
    /// Builds the investigation content from the operator's choices, applying
    /// every business rule and reporting all violations at once. `system` is
    /// `None` when no (known) system was chosen. `number` is the candidate
    /// number; the final number may change at creation time (see
    /// `services::investigations`).
    pub fn build(
        number: InvestigationNumber,
        date: NaiveDate,
        system: Option<&System>,
        log: &OperationsLog,
        selected_row_ids: &[u32],
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

        if selected_row_ids.is_empty() {
            errors.push(FieldError::new("rows", "no_rows_selected"));
        } else if selected_row_ids.len() > MAX_ROWS_PER_INVESTIGATION {
            errors.push(FieldError::new("rows", "too_many_rows"));
        } else if selected_row_ids.iter().any(|id| !log.rows.iter().any(|row| row.id == *id)) {
            errors.push(FieldError::new("rows", "unknown_row"));
        }

        let system = match system {
            Some(system) if errors.is_empty() => system,
            _ => return Err(errors),
        };

        // Keep the workbook's chronological order regardless of click order,
        // and ignore duplicate ids.
        let rows = log
            .rows
            .iter()
            .filter(|row| selected_row_ids.contains(&row.id))
            .map(|row| InvestigationRow {
                time: row.time.clone(),
                from: row.from.clone(),
                to: row.to.clone(),
                description: row.description.clone(),
            })
            .collect();

        Ok(Self {
            number,
            date,
            system: SystemRef { id: system.id.clone(), name: system.name.clone() },
            template: system.template.clone(),
            preliminary_check_url: url,
            rows,
            source_file_name: log.source_file_name.clone(),
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
    /// Where the adapter stored the document (a fictional URL for the mock).
    pub location: String,
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
    use crate::domain::operations_log::OperationsLogRow;
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
                library: "תחקירים".into(),
            },
        }
    }

    pub(crate) fn log() -> OperationsLog {
        let row = |id: u32, time: &str, description: &str| OperationsLogRow {
            id,
            time: time.into(),
            from: "מוקד".into(),
            to: "עמדה 3".into(),
            description: description.into(),
            event_type: "תקלה".into(),
            highlighted: false,
        };
        OperationsLog {
            source_file_name: "log.xlsx".into(),
            sheet_name: "Log".into(),
            rows: vec![row(2, "08:00", "first"), row(3, "08:10", "second"), row(4, "08:20", "third")],
            highlight_detection_available: true,
        }
    }

    fn number() -> InvestigationNumber {
        InvestigationNumber::new(56, 2026).unwrap()
    }

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 23).unwrap()
    }

    const URL: &str = "https://checks.example.com/runs/1";

    #[test]
    fn builds_from_selected_rows_in_log_order() {
        let inv =
            Investigation::build(number(), date(), Some(&system("alpha", true)), &log(), &[4, 2, 4], URL).unwrap();
        let descriptions: Vec<&str> = inv.rows.iter().map(|r| r.description.as_str()).collect();
        assert_eq!(descriptions, vec!["first", "third"]);
        assert_eq!(inv.number.to_string(), "056-2026");
        assert_eq!(inv.preliminary_check_url, URL);
        assert_eq!(inv.source_file_name, "log.xlsx");
    }

    #[test]
    fn uses_the_template_of_the_selected_system() {
        let alpha = Investigation::build(number(), date(), Some(&system("alpha", true)), &log(), &[2], URL).unwrap();
        let bravo = Investigation::build(number(), date(), Some(&system("bravo", true)), &log(), &[2], URL).unwrap();
        assert_eq!(alpha.template.name, "תבנית alpha");
        assert_eq!(bravo.template.name, "תבנית bravo");
        assert_eq!(bravo.system, SystemRef { id: "bravo".into(), name: "מערכת bravo".into() });
    }

    #[test]
    fn rejects_inactive_systems() {
        let errors =
            Investigation::build(number(), date(), Some(&system("old", false)), &log(), &[2], URL).unwrap_err();
        assert_eq!(errors, vec![FieldError::new("systemId", "system_inactive")]);
    }

    #[test]
    fn reports_all_missing_inputs_together() {
        let errors = Investigation::build(number(), date(), None, &log(), &[], "ftp://x.example.com").unwrap_err();
        assert_eq!(
            errors,
            vec![
                FieldError::new("systemId", "required"),
                FieldError::new("preliminaryCheckUrl", "unsupported_scheme"),
                FieldError::new("rows", "no_rows_selected"),
            ]
        );
    }

    #[test]
    fn rejects_rows_that_are_not_in_the_import() {
        let errors =
            Investigation::build(number(), date(), Some(&system("alpha", true)), &log(), &[2, 99], URL).unwrap_err();
        assert_eq!(errors, vec![FieldError::new("rows", "unknown_row")]);
    }

    #[test]
    fn serialises_without_event_type() {
        let inv = Investigation::build(number(), date(), Some(&system("alpha", true)), &log(), &[2], URL).unwrap();
        let json = serde_json::to_value(&inv).unwrap();
        assert_eq!(json["date"], "2026-09-23");
        assert_eq!(json["number"], "056-2026");
        assert!(json["rows"][0].get("eventType").is_none());
    }
}
