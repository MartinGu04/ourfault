//! A presentation-neutral view of an investigation document: titled blocks
//! of labelled values and tables, all plain text.
//!
//! This is the single place where the document's (Hebrew) wording is
//! decided. The in-app preview, the HTML renderer and the PDF renderer all
//! draw the same view, so they never disagree. A view can be built from a
//! completed investigation or, leniently, from an incomplete draft.

use serde::Serialize;

use crate::domain::activity::{ActivityStatus, ActivityType, ActivityTypeKind, LocalDateTime};
use crate::domain::configuration::Configuration;
use crate::domain::draft::DraftContent;
use crate::domain::investigation::Investigation;
use crate::domain::lifecycle::{InvestigationStatus, Lifecycle};
use crate::domain::log_rows::LogRow;
use crate::domain::sections::{Cell, FieldKind, FieldValue, SectionMode};

use super::labels;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentView {
    pub title: String,
    /// The activity name.
    pub subtitle: String,
    /// `None` for drafts: the number is allocated on completion.
    pub number: Option<String>,
    pub status: InvestigationStatus,
    /// Shown prominently on every page of a draft ("not for distribution").
    pub draft_notice: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Block {
    /// Labelled values laid out as a grid.
    Fields {
        title: String,
        fields: Vec<FieldLine>,
    },
    Table {
        title: String,
        columns: Vec<ColumnView>,
        rows: Vec<Vec<String>>,
        empty_text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldLine {
    pub label: String,
    pub value: String,
    /// Left-to-right content (URLs, numbers).
    pub ltr: bool,
    /// Spans the whole width.
    pub wide: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnView {
    pub label: String,
    pub ltr: bool,
}

fn line(label: &str, value: impl Into<String>) -> FieldLine {
    FieldLine { label: label.to_owned(), value: value.into(), ltr: false, wide: false }
}

fn or_missing(value: String) -> String {
    if value.trim().is_empty() {
        labels::MISSING.to_owned()
    } else {
        value
    }
}

/// "2026-09-20T08:15" → "20/09/2026 08:15".
fn format_local(value: LocalDateTime) -> String {
    value.naive().format("%d/%m/%Y %H:%M").to_string()
}

/// Draft times are raw input: formatted when valid, shown as typed otherwise.
fn format_raw_time(value: &str) -> String {
    value.parse::<LocalDateTime>().map(format_local).unwrap_or_else(|_| or_missing(value.to_owned()))
}

/// RFC 3339 timestamp → "23/09/2026 10:00" (local time as recorded).
pub fn format_timestamp(value: &str) -> String {
    match (value.get(0..10), value.get(11..16)) {
        (Some(date), Some(time)) if date.len() == 10 => {
            format!("{}/{}/{} {time}", &date[8..10], &date[5..7], &date[0..4])
        }
        _ => value.to_owned(),
    }
}

fn yes_no(value: Option<bool>) -> String {
    match value {
        Some(true) => labels::YES.to_owned(),
        Some(false) => labels::NO.to_owned(),
        None => labels::MISSING.to_owned(),
    }
}

fn activity_type_label(activity_type: &ActivityType) -> String {
    match activity_type {
        ActivityType::Other { description } => {
            format!("{} – {description}", labels::activity_type(ActivityTypeKind::Other))
        }
        other => labels::activity_type(other.kind()).to_owned(),
    }
}

fn chronology(rows: &[LogRow]) -> Block {
    Block::Table {
        title: labels::CHRONOLOGY.to_owned(),
        columns: vec![
            ColumnView { label: labels::TIME.to_owned(), ltr: true },
            ColumnView { label: labels::FROM.to_owned(), ltr: false },
            ColumnView { label: labels::TO.to_owned(), ltr: false },
            ColumnView { label: labels::DESCRIPTION.to_owned(), ltr: false },
        ],
        rows: rows
            .iter()
            .map(|row| vec![row.time.clone(), row.from.clone(), row.to.clone(), row.description.clone()])
            .collect(),
        empty_text: labels::NO_ROWS.to_owned(),
    }
}

fn checks(url: &str) -> Block {
    Block::Fields {
        title: labels::CHECKS.to_owned(),
        fields: vec![FieldLine {
            label: labels::CHECKS_LINK.to_owned(),
            value: or_missing(url.to_owned()),
            ltr: !url.trim().is_empty(),
            wide: true,
        }],
    }
}

fn times(planned: [String; 2], actual: [String; 2]) -> [Block; 2] {
    let [planned_start, planned_end] = planned;
    let [actual_start, actual_end] = actual;
    [
        Block::Fields {
            title: labels::PLANNED.to_owned(),
            fields: vec![line(labels::START, planned_start), line(labels::END, planned_end)],
        },
        Block::Fields {
            title: labels::ACTUAL.to_owned(),
            fields: vec![line(labels::START, actual_start), line(labels::END, actual_end)],
        },
    ]
}

/// A technical section: a table for repeating sections, a field grid for
/// single-record sections.
fn section_block(name: &str, mode: SectionMode, columns: Vec<ColumnView>, rows: Vec<Vec<String>>) -> Block {
    match mode {
        SectionMode::Repeating => {
            Block::Table { title: name.to_owned(), columns, rows, empty_text: labels::NO_ROWS.to_owned() }
        }
        SectionMode::Single => {
            let values = rows.into_iter().next().unwrap_or_default();
            Block::Fields {
                title: name.to_owned(),
                fields: columns
                    .into_iter()
                    .enumerate()
                    .map(|(index, column)| FieldLine {
                        label: column.label,
                        value: or_missing(values.get(index).cloned().unwrap_or_default()),
                        ltr: column.ltr,
                        wide: false,
                    })
                    .collect(),
            }
        }
    }
}

fn cell_text(cell: &Cell) -> String {
    match cell {
        Cell::Empty => String::new(),
        Cell::Text(text) | Cell::Number(text) | Cell::Choice(text) => text.clone(),
        Cell::Boolean(value) => yes_no(Some(*value)),
        Cell::Choices(items) => items.join(", "),
        Cell::Station(reference) | Cell::System(reference) => reference.name.clone(),
    }
}

impl DocumentView {
    pub fn from_investigation(investigation: &Investigation, lifecycle: &Lifecycle) -> Self {
        let activity = &investigation.activity;

        let mut blocks = vec![Block::Fields {
            title: labels::ACTIVITY_DETAILS.to_owned(),
            fields: vec![
                line(labels::ACTIVITY_NAME, activity.name.clone()),
                line(labels::ACTIVITY_TYPE, activity_type_label(&activity.activity_type)),
                line(labels::SYSTEMS, activity.system_names().join(", ")),
                line(labels::ACTIVITY_STATUS, labels::activity_status(activity.status)),
                line(labels::NIGHT_ACTIVITY, yes_no(Some(activity.night_activity))),
                line(labels::SENIOR_STAFFING, yes_no(Some(activity.senior_staffing))),
            ],
        }];
        blocks.extend(times(
            [format_local(activity.planned.start), format_local(activity.planned.end)],
            [
                format_local(activity.actual.start),
                activity.actual.end.map(format_local).unwrap_or_else(|| labels::NOT_ENDED.to_owned()),
            ],
        ));
        for section in &investigation.sections {
            let columns = section
                .columns
                .iter()
                .map(|column| ColumnView { label: column.label.clone(), ltr: column.kind == FieldKind::Number })
                .collect();
            let rows = section.rows.iter().map(|row| row.iter().map(cell_text).collect()).collect();
            blocks.push(section_block(&section.name, section.mode, columns, rows));
        }
        blocks.push(chronology(&investigation.rows));
        blocks.push(checks(&investigation.preliminary_check_url));

        let mut status = vec![
            line(labels::INVESTIGATION_STATUS, labels::investigation_status(lifecycle.status)),
            line(labels::COMPLETED_AT, format_timestamp(&lifecycle.completed_at)),
        ];
        if let Some(distributed_at) = &lifecycle.distributed_at {
            status.push(line(labels::DISTRIBUTED_AT, format_timestamp(distributed_at)));
        }
        blocks.push(Block::Fields { title: labels::INVESTIGATION.to_owned(), fields: status });

        Self {
            title: format!("{} {}", labels::INVESTIGATION, investigation.number),
            subtitle: activity.name.clone(),
            number: Some(investigation.number.to_string()),
            status: lifecycle.status,
            draft_notice: None,
            blocks,
        }
    }

    /// A best-effort view of an incomplete draft. Missing values are shown
    /// as missing, references are resolved against the current
    /// configuration, and the document is always marked as a draft.
    pub fn from_draft(content: &DraftContent, configuration: &Configuration) -> Self {
        let activity = &content.activity;
        let system_name = |id: &String| configuration.system(id).map(|s| s.name.clone()).unwrap_or_else(|| id.clone());
        let activity_type = match activity.activity_type {
            None => labels::MISSING.to_owned(),
            Some(ActivityTypeKind::Other) if !activity.activity_type_other.trim().is_empty() => {
                format!("{} – {}", labels::activity_type(ActivityTypeKind::Other), activity.activity_type_other.trim())
            }
            Some(kind) => labels::activity_type(kind).to_owned(),
        };
        let actual_end = match (activity.status, activity.actual_end.trim().is_empty()) {
            (Some(ActivityStatus::Active), true) => labels::NOT_ENDED.to_owned(),
            _ => format_raw_time(&activity.actual_end),
        };

        let mut blocks = vec![Block::Fields {
            title: labels::ACTIVITY_DETAILS.to_owned(),
            fields: vec![
                line(labels::ACTIVITY_NAME, or_missing(activity.name.trim().to_owned())),
                line(labels::ACTIVITY_TYPE, activity_type),
                line(
                    labels::SYSTEMS,
                    or_missing(activity.system_ids.iter().map(system_name).collect::<Vec<_>>().join(", ")),
                ),
                line(
                    labels::ACTIVITY_STATUS,
                    activity.status.map(labels::activity_status).unwrap_or(labels::MISSING).to_owned(),
                ),
                line(labels::NIGHT_ACTIVITY, yes_no(activity.night_activity)),
                line(labels::SENIOR_STAFFING, yes_no(activity.senior_staffing)),
            ],
        }];
        blocks.extend(times(
            [format_raw_time(&activity.planned_start), format_raw_time(&activity.planned_end)],
            [format_raw_time(&activity.actual_start), actual_end],
        ));

        for section in configuration.active_sections() {
            let records = content.sections.get(&section.id).map(Vec::as_slice).unwrap_or_default();
            let columns = section
                .fields
                .iter()
                .map(|field| ColumnView { label: field.label.clone(), ltr: field.kind == FieldKind::Number })
                .collect();
            let rows = records
                .iter()
                .map(|record| {
                    section
                        .fields
                        .iter()
                        .map(|field| match (field.kind, record.get(&field.id)) {
                            (_, None) => String::new(),
                            (_, Some(FieldValue::Bool(value))) => yes_no(Some(*value)),
                            (_, Some(FieldValue::List(items))) => items.join(", "),
                            (FieldKind::Station, Some(FieldValue::Text(id))) => {
                                configuration.station(id).map(|s| s.name.clone()).unwrap_or_else(|| id.clone())
                            }
                            (FieldKind::System, Some(FieldValue::Text(id))) => system_name(id),
                            (_, Some(FieldValue::Text(text))) => text.clone(),
                        })
                        .collect()
                })
                .collect();
            blocks.push(section_block(&section.name, section.mode, columns, rows));
        }
        blocks.push(chronology(&content.rows));
        blocks.push(checks(&content.preliminary_check_url));

        Self {
            title: labels::DRAFT_TITLE.to_owned(),
            subtitle: or_missing(activity.name.trim().to_owned()),
            number: None,
            status: InvestigationStatus::Draft,
            draft_notice: Some(labels::DRAFT_NOTICE.to_owned()),
            blocks,
        }
    }

    /// Every piece of text in the view, for font coverage checks.
    #[cfg(test)]
    pub fn all_text(&self) -> Vec<String> {
        let mut text = vec![self.title.clone(), self.subtitle.clone()];
        text.extend(self.draft_notice.clone());
        for block in &self.blocks {
            match block {
                Block::Fields { title, fields } => {
                    text.push(title.clone());
                    for field in fields {
                        text.push(field.label.clone());
                        text.push(field.value.clone());
                    }
                }
                Block::Table { title, columns, rows, empty_text } => {
                    text.push(title.clone());
                    text.push(empty_text.clone());
                    text.extend(columns.iter().map(|c| c.label.clone()));
                    text.extend(rows.iter().flatten().cloned());
                }
            }
        }
        text
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::domain::configuration::tests::configuration;
    use crate::domain::investigation::tests::{content, number};
    use crate::domain::investigation::{Publication, PublicationKind, PublishedInvestigation};

    pub(crate) fn published() -> PublishedInvestigation {
        let mut lifecycle = Lifecycle::completed("2026-09-23T10:00:00+03:00", "op");
        lifecycle.record_distribution("2026-09-23T11:30:00+03:00", "op").unwrap();
        PublishedInvestigation {
            investigation: Investigation::build(number(), &content(), &configuration()).unwrap(),
            lifecycle,
            publication: Publication {
                destination: PublicationKind::SharePoint,
                url: "https://sharepoint.example.com/sites/ops/Lists/Investigations/DispForm.aspx?ID=1".into(),
                reference: "1".into(),
            },
        }
    }

    pub(crate) fn view_of(published: &PublishedInvestigation) -> DocumentView {
        DocumentView::from_investigation(&published.investigation, &published.lifecycle)
    }

    fn titles(view: &DocumentView) -> Vec<String> {
        view.blocks
            .iter()
            .map(|block| match block {
                Block::Fields { title, .. } | Block::Table { title, .. } => title.clone(),
            })
            .collect()
    }

    #[test]
    fn completed_investigations_list_every_part_in_order() {
        let view = view_of(&published());
        assert_eq!(view.title, "תחקיר 056-2026");
        assert_eq!(view.subtitle, "בלט רומני");
        assert_eq!(view.draft_notice, None);
        assert_eq!(
            titles(&view),
            vec![
                "פרטי הפעילות",
                "תכנון",
                "ביצוע בפועל",
                "קישורים",
                "כלים",
                "השתלשלות אירועים",
                "בדיקות מקדימות",
                "תחקיר"
            ]
        );
        let Block::Table { columns, rows, .. } = &view.blocks[4] else { panic!("vehicles is a table") };
        let labels: Vec<&str> = columns.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, vec!["כותרת 1", "כותרת 2", "מס׳ זנב", "מס׳ קרון", "מקטע"]);
        assert_eq!(rows[1], vec!["x2", "", "4X-102", "", "ג"]);
        let Block::Table { rows, .. } = &view.blocks[3] else { panic!("links is a table") };
        assert_eq!(rows[0][2], "תחנת אורן", "stations are shown by name");
        let Block::Fields { fields, .. } = &view.blocks[7] else { panic!() };
        assert_eq!(fields[0].value, "הופץ");
        assert_eq!(fields[2].value, "23/09/2026 11:30");
    }

    #[test]
    fn drafts_are_marked_and_tolerate_missing_values() {
        let mut draft = DraftContent::default();
        draft.activity.system_ids = vec!["alpha".into(), "unknown".into()];
        draft.activity.planned_start = "2026-09-20T08:00".into();
        draft.activity.actual_start = "garbage".into();
        let view = DocumentView::from_draft(&draft, &configuration());
        assert_eq!(view.number, None);
        assert_eq!(view.status, InvestigationStatus::Draft);
        assert_eq!(view.draft_notice.as_deref(), Some("טיוטה — לא להפצה"));
        let Block::Fields { fields, .. } = &view.blocks[0] else { panic!() };
        assert_eq!(fields[0].value, "—");
        assert_eq!(fields[2].value, "מערכת alpha, unknown");
        let Block::Fields { fields, .. } = &view.blocks[1] else { panic!() };
        assert_eq!(fields[0].value, "20/09/2026 08:00");
        let Block::Fields { fields, .. } = &view.blocks[2] else { panic!() };
        assert_eq!(fields[0].value, "garbage", "shown as typed");
    }

    #[test]
    fn formats_timestamps_without_time_zone_conversion() {
        assert_eq!(format_timestamp("2026-09-23T23:59:00-05:00"), "23/09/2026 23:59");
        assert_eq!(format_timestamp("x"), "x");
    }
}
