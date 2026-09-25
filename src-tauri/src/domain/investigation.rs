//! A completed investigation: the structured source of truth. Renderers
//! (HTML, PDF) and publishers derive everything else from it; it is never
//! collapsed into markup.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::activity::{Activity, ActivityTypeKind};
use super::configuration::Configuration;
use super::draft::DraftContent;
use super::investigation_number::InvestigationNumber;
use super::lifecycle::{InvestigationStatus, Lifecycle};
use super::log_rows::{validate_rows, LogRow};
use super::sections::{validate_sections, References, TechnicalSection};
use super::validation::{validate_preliminary_check_url, FieldError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Investigation {
    pub number: InvestigationNumber,
    pub activity: Activity,
    /// Technical sections in configured order, with their definitions
    /// snapshotted.
    pub sections: Vec<TechnicalSection>,
    /// The event chronology: operations-log rows in the operator's order.
    pub rows: Vec<LogRow>,
    pub preliminary_check_url: String,
    /// The draft this investigation was created from. Publishers enforce that
    /// one draft creates at most one investigation (see
    /// `InvestigationPublisher::publish`), which makes completion idempotent.
    /// `None` only for investigations that did not come from a draft
    /// (e.g. demo history).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_draft_id: Option<String>,
}

impl Investigation {
    /// Validates a draft against the current configuration and builds the
    /// investigation under the candidate `number`. All violations are
    /// reported together.
    pub fn build(
        number: InvestigationNumber,
        content: &DraftContent,
        configuration: &Configuration,
    ) -> Result<Self, Vec<FieldError>> {
        let mut errors = Vec::new();
        let activity = content.activity.validate(configuration, &mut errors);

        let involved = activity.as_ref().map(|activity| activity.systems.clone()).unwrap_or_default();
        let sections = validate_sections(
            &configuration.active_sections(),
            &content.sections,
            &References { stations: &configuration.stations, systems: &involved },
            &mut errors,
        );

        let rows = validate_rows(&content.rows).unwrap_or_else(|code| {
            errors.push(FieldError::new("rows", code));
            Vec::new()
        });
        let preliminary_check_url =
            validate_preliminary_check_url(&content.preliminary_check_url).unwrap_or_else(|code| {
                errors.push(FieldError::new("preliminaryCheckUrl", code));
                String::new()
            });

        match activity {
            Some(activity) if errors.is_empty() => {
                Ok(Self { number, activity, sections, rows, preliminary_check_url, source_draft_id: None })
            }
            _ => Err(errors),
        }
    }

    pub fn with_number(mut self, number: InvestigationNumber) -> Self {
        self.number = number;
        self
    }

    pub fn with_source_draft(mut self, draft_id: &str) -> Self {
        self.source_draft_id = Some(draft_id.to_owned());
        self
    }
}

/// Where an investigation was published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    pub destination: PublicationKind,
    /// Address of the published investigation (SharePoint item, or file URL).
    pub url: String,
    /// Destination-specific reference, e.g. the SharePoint list item id.
    pub reference: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PublicationKind {
    SharePoint,
    SharedFolder,
}

/// An investigation as read back from its publication destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedInvestigation {
    pub investigation: Investigation,
    pub lifecycle: Lifecycle,
    pub publication: Publication,
}

/// The metadata a list or dashboard shows (for SharePoint: the list columns
/// מס׳ תחקיר | מערכות מופעלות | שם משימה | תאריך, plus type and status).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationSummary {
    pub number: InvestigationNumber,
    pub activity_name: String,
    pub systems: Vec<String>,
    pub date: NaiveDate,
    pub activity_type: ActivityTypeKind,
    pub status: InvestigationStatus,
}

impl From<&PublishedInvestigation> for InvestigationSummary {
    fn from(published: &PublishedInvestigation) -> Self {
        let activity = &published.investigation.activity;
        Self {
            number: published.investigation.number,
            activity_name: activity.name.clone(),
            systems: activity.system_names(),
            date: activity.date(),
            activity_type: activity.activity_type.kind(),
            status: published.lifecycle.status,
        }
    }
}

impl InvestigationSummary {
    /// Simple case-insensitive match on activity name and system names. The
    /// number is matched separately (exactly, after lenient parsing).
    pub fn matches_text(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        !query.is_empty()
            && (self.activity_name.to_lowercase().contains(&query)
                || self.systems.iter().any(|system| system.to_lowercase().contains(&query))
                || self.number.to_string().contains(&query))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::domain::activity::tests::activity_input;
    use crate::domain::configuration::tests::configuration;
    use crate::domain::sections::tests::{record, text};
    use crate::domain::sections::SectionValues;

    pub(crate) fn rows() -> Vec<LogRow> {
        let row = |time: &str, description: &str| LogRow {
            time: time.into(),
            from: "מוקד".into(),
            to: "עמדה 3".into(),
            description: description.into(),
        };
        vec![row("08:00", "first"), row("08:10", "second")]
    }

    /// A complete draft for the test configuration.
    pub(crate) fn content() -> DraftContent {
        DraftContent {
            activity: activity_input(),
            sections: SectionValues::from([
                ("links".into(), vec![record(&[("f1", text("קו 1")), ("f3", text("st-1"))])]),
                (
                    "vehicles".into(),
                    vec![
                        record(&[("f1", text("x1")), ("f3", text("4X-101"))]),
                        record(&[("f1", text("x2")), ("f3", text("4X-102")), ("f5", text("ג"))]),
                    ],
                ),
            ]),
            rows: rows(),
            preliminary_check_url: "https://checks.example.com/runs/1".into(),
        }
    }

    pub(crate) fn number() -> InvestigationNumber {
        InvestigationNumber::new(56, 2026).unwrap()
    }

    #[test]
    fn builds_a_multi_system_investigation_from_a_complete_draft() {
        let inv = Investigation::build(number(), &content(), &configuration()).unwrap();
        assert_eq!(inv.number.to_string(), "056-2026");
        assert_eq!(inv.activity.system_names(), vec!["מערכת alpha", "מערכת beta"]);
        let sections: Vec<(&str, usize)> = inv.sections.iter().map(|s| (s.id.as_str(), s.rows.len())).collect();
        assert_eq!(sections, vec![("links", 1), ("vehicles", 2)], "configured order");
        let descriptions: Vec<&str> = inv.rows.iter().map(|r| r.description.as_str()).collect();
        assert_eq!(descriptions, vec!["first", "second"]);
    }

    #[test]
    fn reports_problems_from_every_part_together() {
        let mut draft = content();
        draft.activity.name.clear();
        draft.sections.remove("links");
        draft.sections.insert("links".into(), vec![record(&[("f1", text("x"))])]);
        draft.rows.clear();
        draft.preliminary_check_url = "ftp://x.example.com".into();
        let errors = Investigation::build(number(), &draft, &configuration()).unwrap_err();
        let fields: Vec<(&str, &str)> = errors.iter().map(|e| (e.field.as_str(), e.code)).collect();
        assert_eq!(
            fields,
            vec![
                ("activityName", "required"),
                ("sections.links.0.f3", "required"),
                ("rows", "no_rows"),
                ("preliminaryCheckUrl", "unsupported_scheme"),
            ]
        );
    }

    #[test]
    fn system_fields_can_only_reference_involved_systems() {
        let mut config = configuration();
        config.sections[1].fields.push(crate::domain::sections::tests::field(
            "f9",
            "מערכת",
            crate::domain::sections::FieldKind::System,
            false,
        ));
        let mut draft = content();
        draft.sections.get_mut("vehicles").unwrap()[0].insert("f9".into(), text("retired"));
        let errors = Investigation::build(number(), &draft, &config).unwrap_err();
        assert_eq!(errors, vec![FieldError::new("sections.vehicles.0.f9", "system_not_involved")]);
    }

    #[test]
    fn summary_matches_name_system_and_number() {
        let inv = Investigation::build(number(), &content(), &configuration()).unwrap();
        let published = PublishedInvestigation {
            investigation: inv,
            lifecycle: Lifecycle::completed("t", "op"),
            publication: Publication {
                destination: PublicationKind::SharePoint,
                url: "u".into(),
                reference: "1".into(),
            },
        };
        let summary = InvestigationSummary::from(&published);
        assert_eq!(summary.date.to_string(), "2026-09-20");
        assert!(summary.matches_text("רומני"));
        assert!(summary.matches_text("BETA"));
        assert!(summary.matches_text("056"));
        assert!(!summary.matches_text("gamma"));
        assert!(!summary.matches_text("  "));
    }
}
