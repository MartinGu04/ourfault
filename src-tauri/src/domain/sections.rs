//! Configurable technical sections.
//!
//! Administrators define structured sections (single record or repeating
//! rows) with typed fields. This is deliberately not a document editor: the
//! field types are a small closed set, and values are data, never markup.
//!
//! * [`SectionDefinition`] / [`FieldDefinition`] – configuration. Order is the
//!   order of the vectors.
//! * [`SectionValues`] – what the operator entered in a draft, keyed by
//!   section and field id and interpreted through the definitions.
//! * [`TechnicalSection`] – the validated snapshot stored with a completed
//!   investigation. It carries its own labels and resolved names, so later
//!   configuration changes never alter existing investigations.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::activity::SystemRef;
use super::log_rows::MAX_CELL_CHARS;
use super::station::Station;
use super::validation::{required_text, FieldError};

pub const MAX_SECTION_NAME_CHARS: usize = 80;
pub const MAX_FIELD_LABEL_CHARS: usize = 80;
pub const MAX_FIELDS: usize = 30;
pub const MAX_OPTIONS: usize = 50;
pub const MAX_SECTION_ROWS: usize = 200;

/// The supported field types. Intentionally small and explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldKind {
    Text,
    Number,
    Boolean,
    SingleSelect,
    MultiSelect,
    /// A station from the centrally managed station list.
    Station,
    /// One of the systems involved in the investigation.
    System,
}

impl FieldKind {
    pub fn has_options(self) -> bool {
        matches!(self, FieldKind::SingleSelect | FieldKind::MultiSelect)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SectionMode {
    /// One record, shown as a form.
    Single,
    /// Any number of rows, shown as a table.
    Repeating,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDefinition {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,
    pub required: bool,
    /// Inactive fields are hidden from new investigations.
    pub active: bool,
    /// Choices for select fields; empty for every other kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionDefinition {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub mode: SectionMode,
    pub fields: Vec<FieldDefinition>,
}

impl SectionDefinition {
    /// The section as operators see it: only active fields.
    pub fn with_active_fields(&self) -> Self {
        Self { fields: self.fields.iter().filter(|field| field.active).cloned().collect(), ..self.clone() }
    }
}

// ---- Administration ---------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInput {
    /// `None` adds a new field.
    pub id: Option<String>,
    pub label: String,
    pub kind: FieldKind,
    pub required: bool,
    pub active: bool,
    #[serde(default)]
    pub options: Vec<String>,
}

/// A whole section as edited by an administrator: fields in their new order.
/// Fields missing from the input are removed from the definition (completed
/// investigations keep their own snapshot).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionInput {
    /// `None` creates a new section.
    pub id: Option<String>,
    pub name: String,
    pub mode: SectionMode,
    pub fields: Vec<FieldInput>,
}

impl SectionInput {
    /// Validates the input and applies it to `existing` (or to a new, active
    /// section with id `new_id`). New fields get ids that are unique within
    /// the section; existing ids must belong to the section.
    pub fn apply(
        &self,
        existing: Option<&SectionDefinition>,
        new_id: String,
    ) -> Result<SectionDefinition, Vec<FieldError>> {
        let mut errors = Vec::new();
        let name = required_text(&self.name, MAX_SECTION_NAME_CHARS).unwrap_or_else(|code| {
            errors.push(FieldError::new("name", code));
            String::new()
        });

        if self.fields.is_empty() {
            errors.push(FieldError::new("fields", "required"));
        } else if self.fields.len() > MAX_FIELDS {
            errors.push(FieldError::new("fields", "too_many"));
        }

        let existing_fields = existing.map(|section| section.fields.as_slice()).unwrap_or_default();
        let mut next_number = existing_fields.iter().filter_map(|f| numeric_suffix(&f.id, "f")).max().unwrap_or(0);
        let mut fields: Vec<FieldDefinition> = Vec::new();
        for (index, input) in self.fields.iter().enumerate() {
            let prefix = format!("fields.{index}");
            let id = match &input.id {
                Some(id) if existing_fields.iter().any(|field| &field.id == id) => id.clone(),
                Some(_) => {
                    errors.push(FieldError::new(&prefix, "unknown_field"));
                    continue;
                }
                None => {
                    next_number += 1;
                    format!("f{next_number}")
                }
            };
            let label = match required_text(&input.label, MAX_FIELD_LABEL_CHARS) {
                Ok(label) if fields.iter().any(|f| f.label.to_lowercase() == label.to_lowercase()) => {
                    errors.push(FieldError::new(&format!("{prefix}.label"), "duplicate_label"));
                    label
                }
                Ok(label) => label,
                Err(code) => {
                    errors.push(FieldError::new(&format!("{prefix}.label"), code));
                    String::new()
                }
            };
            let options = if input.kind.has_options() {
                validate_options(&input.options, &format!("{prefix}.options"), &mut errors)
            } else {
                Vec::new()
            };
            fields.push(FieldDefinition {
                id,
                label,
                kind: input.kind,
                required: input.required,
                active: input.active,
                options,
            });
        }

        if !errors.is_empty() {
            return Err(errors);
        }
        let (id, active) = existing.map(|section| (section.id.clone(), section.active)).unwrap_or((new_id, true));
        Ok(SectionDefinition { id, name, active, mode: self.mode, fields })
    }
}

fn validate_options(options: &[String], field: &str, errors: &mut Vec<FieldError>) -> Vec<String> {
    let mut valid: Vec<String> = Vec::new();
    for option in options.iter().filter(|option| !option.trim().is_empty()) {
        match required_text(option, MAX_FIELD_LABEL_CHARS) {
            Ok(option) if !valid.contains(&option) => valid.push(option),
            Ok(_) => {}
            Err(code) => {
                errors.push(FieldError::new(field, code));
                return Vec::new();
            }
        }
    }
    if valid.is_empty() {
        errors.push(FieldError::new(field, "required"));
    } else if valid.len() > MAX_OPTIONS {
        errors.push(FieldError::new(field, "too_many"));
    }
    valid
}

/// `n` for ids of the form `{prefix}{n}` or `{prefix}-{n}`.
pub(crate) fn numeric_suffix(id: &str, prefix: &str) -> Option<u32> {
    id.strip_prefix(prefix).map(|rest| rest.trim_start_matches('-')).and_then(|n| n.parse().ok())
}

// ---- Values -----------------------------------------------------------------

/// A value as entered in a draft. Its meaning comes from the field kind:
/// text, number (as typed), single choice, station id and system id are
/// strings; booleans are booleans; multi-select is a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Bool(bool),
    List(Vec<String>),
    Text(String),
}

/// One record (a row, or the single record of a section): field id → value.
pub type Record = BTreeMap<String, FieldValue>;

/// All section values of a draft: section id → records.
pub type SectionValues = BTreeMap<String, Vec<Record>>;

/// A reference resolved to its name at the time of completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedRef {
    pub id: String,
    pub name: String,
}

/// A validated, typed value in a completed investigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum Cell {
    Empty,
    Text(String),
    /// The number exactly as entered (validated), to avoid float formatting.
    Number(String),
    Boolean(bool),
    Choice(String),
    Choices(Vec<String>),
    Station(NamedRef),
    System(NamedRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,
}

/// A technical section as stored with a completed investigation. Every row
/// has one cell per column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalSection {
    pub id: String,
    pub name: String,
    pub mode: SectionMode,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
}

/// What reference fields may point at.
pub struct References<'a> {
    pub stations: &'a [Station],
    /// The systems involved in the investigation.
    pub systems: &'a [SystemRef],
}

/// Validates the values of every given section (already filtered to active
/// sections and fields) and returns the snapshots. Errors are keyed
/// `sections.{section}`, `sections.{section}.{row}` and
/// `sections.{section}.{row}.{field}`. Values of unknown or removed fields
/// are ignored.
pub fn validate_sections(
    definitions: &[SectionDefinition],
    values: &SectionValues,
    references: &References<'_>,
    errors: &mut Vec<FieldError>,
) -> Vec<TechnicalSection> {
    let empty = Record::new();
    definitions
        .iter()
        .map(|definition| {
            let key = format!("sections.{}", definition.id);
            let records = values.get(&definition.id).map(Vec::as_slice).unwrap_or_default();
            let records: Vec<&Record> = match definition.mode {
                SectionMode::Single => {
                    if records.len() > 1 {
                        errors.push(FieldError::new(&key, "single_record"));
                    }
                    vec![records.first().unwrap_or(&empty)]
                }
                SectionMode::Repeating => {
                    if records.len() > MAX_SECTION_ROWS {
                        errors.push(FieldError::new(&key, "too_many_rows"));
                    }
                    records.iter().take(MAX_SECTION_ROWS).collect()
                }
            };
            let rows = records
                .iter()
                .enumerate()
                .map(|(index, record)| {
                    let row_key = format!("{key}.{index}");
                    let cells: Vec<Cell> = definition
                        .fields
                        .iter()
                        .map(|field| {
                            let cell = validate_cell(field, record.get(&field.id), references).unwrap_or_else(|code| {
                                errors.push(FieldError::new(&format!("{row_key}.{}", field.id), code));
                                Cell::Empty
                            });
                            if cell == Cell::Empty && field.required && record.get(&field.id).is_none_or(is_blank) {
                                errors.push(FieldError::new(&format!("{row_key}.{}", field.id), "required"));
                            }
                            cell
                        })
                        .collect();
                    let blank = definition.fields.iter().all(|field| record.get(&field.id).is_none_or(is_blank));
                    if definition.mode == SectionMode::Repeating && blank {
                        errors.push(FieldError::new(&row_key, "empty_row"));
                    }
                    cells
                })
                .collect();
            TechnicalSection {
                id: definition.id.clone(),
                name: definition.name.clone(),
                mode: definition.mode,
                columns: definition
                    .fields
                    .iter()
                    .map(|field| Column { id: field.id.clone(), label: field.label.clone(), kind: field.kind })
                    .collect(),
                rows,
            }
        })
        .collect()
}

fn is_blank(value: &FieldValue) -> bool {
    match value {
        FieldValue::Bool(_) => false,
        FieldValue::List(items) => items.iter().all(|item| item.trim().is_empty()),
        FieldValue::Text(text) => text.trim().is_empty(),
    }
}

/// Converts one entered value into a typed cell. Blank values become
/// [`Cell::Empty`]; whether that is allowed is decided by the caller.
fn validate_cell(
    field: &FieldDefinition,
    value: Option<&FieldValue>,
    references: &References<'_>,
) -> Result<Cell, &'static str> {
    let Some(value) = value.filter(|value| !is_blank(value)) else {
        return Ok(Cell::Empty);
    };
    match (field.kind, value) {
        (FieldKind::Boolean, FieldValue::Bool(value)) => Ok(Cell::Boolean(*value)),
        (FieldKind::MultiSelect, FieldValue::List(items)) => {
            let mut chosen: Vec<String> = Vec::new();
            for item in items.iter().map(|item| item.trim()).filter(|item| !item.is_empty()) {
                if !field.options.iter().any(|option| option == item) {
                    return Err("unknown_option");
                }
                if !chosen.iter().any(|c| c == item) {
                    chosen.push(item.to_owned());
                }
            }
            Ok(Cell::Choices(chosen))
        }
        (_, FieldValue::Text(text)) => {
            let text = text.trim();
            if text.chars().count() > MAX_CELL_CHARS {
                return Err("too_long");
            }
            match field.kind {
                FieldKind::Text => Ok(Cell::Text(text.to_owned())),
                FieldKind::Number if is_number(text) => Ok(Cell::Number(text.to_owned())),
                FieldKind::Number => Err("invalid_number"),
                FieldKind::SingleSelect if field.options.iter().any(|option| option == text) => {
                    Ok(Cell::Choice(text.to_owned()))
                }
                FieldKind::SingleSelect => Err("unknown_option"),
                FieldKind::Station => match references.stations.iter().find(|station| station.id == text) {
                    Some(station) if station.active => {
                        Ok(Cell::Station(NamedRef { id: station.id.clone(), name: station.name.clone() }))
                    }
                    Some(_) => Err("station_inactive"),
                    None => Err("unknown_station"),
                },
                FieldKind::System => match references.systems.iter().find(|system| system.id == text) {
                    Some(system) => Ok(Cell::System(NamedRef { id: system.id.clone(), name: system.name.clone() })),
                    None => Err("system_not_involved"),
                },
                FieldKind::Boolean | FieldKind::MultiSelect => Err("invalid_value"),
            }
        }
        _ => Err("invalid_value"),
    }
}

/// A plain decimal number: optional minus, digits, optional fraction.
fn is_number(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, "0"));
    let all_digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    all_digits(whole) && all_digits(fraction) && whole.len() <= 15 && fraction.len() <= 6
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn field(id: &str, label: &str, kind: FieldKind, required: bool) -> FieldDefinition {
        let options = if kind.has_options() { vec!["א".into(), "ב".into(), "ג".into()] } else { vec![] };
        FieldDefinition { id: id.into(), label: label.into(), kind, required, active: true, options }
    }

    /// Mirrors the "column 1 | column 2 | tail | trailer | segment" section.
    pub(crate) fn vehicles_section() -> SectionDefinition {
        SectionDefinition {
            id: "vehicles".into(),
            name: "כלים".into(),
            active: true,
            mode: SectionMode::Repeating,
            fields: vec![
                field("f1", "כותרת 1", FieldKind::Text, true),
                field("f2", "כותרת 2", FieldKind::Text, false),
                field("f3", "מס׳ זנב", FieldKind::Text, true),
                field("f4", "מס׳ קרון", FieldKind::Text, false),
                field("f5", "מקטע", FieldKind::SingleSelect, false),
            ],
        }
    }

    pub(crate) fn stations_section() -> SectionDefinition {
        SectionDefinition {
            id: "links".into(),
            name: "קישורים".into(),
            active: true,
            mode: SectionMode::Repeating,
            fields: vec![
                field("f1", "כותרת 1", FieldKind::Text, true),
                field("f2", "כותרת 2", FieldKind::Text, false),
                field("f3", "תחנה", FieldKind::Station, true),
            ],
        }
    }

    fn conditions_section() -> SectionDefinition {
        SectionDefinition {
            id: "conditions".into(),
            name: "תנאים".into(),
            active: true,
            mode: SectionMode::Single,
            fields: vec![
                field("f1", "משתתפים", FieldKind::Number, true),
                field("f2", "תקלה", FieldKind::Boolean, true),
                field("f3", "סוגים", FieldKind::MultiSelect, false),
                field("f4", "מערכת", FieldKind::System, false),
            ],
        }
    }

    fn stations() -> Vec<Station> {
        vec![
            Station { id: "st-1".into(), name: "תחנת אורן".into(), active: true },
            Station { id: "st-2".into(), name: "תחנת גפן".into(), active: false },
        ]
    }

    fn systems() -> Vec<SystemRef> {
        vec![SystemRef { id: "alpha".into(), name: "מערכת אלפא".into() }]
    }

    pub(crate) fn text(value: &str) -> FieldValue {
        FieldValue::Text(value.into())
    }

    pub(crate) fn record(values: &[(&str, FieldValue)]) -> Record {
        values.iter().map(|(id, value)| (id.to_string(), value.clone())).collect()
    }

    fn validate(
        definitions: &[SectionDefinition],
        values: SectionValues,
    ) -> Result<Vec<TechnicalSection>, Vec<FieldError>> {
        let stations = stations();
        let systems = systems();
        let mut errors = Vec::new();
        let sections = validate_sections(
            definitions,
            &values,
            &References { stations: &stations, systems: &systems },
            &mut errors,
        );
        if errors.is_empty() {
            Ok(sections)
        } else {
            Err(errors)
        }
    }

    fn codes(errors: &[FieldError]) -> Vec<(&str, &str)> {
        errors.iter().map(|e| (e.field.as_str(), e.code)).collect()
    }

    #[test]
    fn repeated_rows_keep_column_and_row_order() {
        let values = SectionValues::from([(
            "vehicles".into(),
            vec![
                record(&[("f1", text("x1")), ("f3", text("4X-101")), ("f5", text("ב"))]),
                record(&[("f5", text("א")), ("f3", text("4X-102")), ("f1", text("x2")), ("f4", text("7"))]),
            ],
        )]);
        let sections = validate(&[vehicles_section()], values).unwrap();
        let section = &sections[0];
        let labels: Vec<&str> = section.columns.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, vec!["כותרת 1", "כותרת 2", "מס׳ זנב", "מס׳ קרון", "מקטע"], "segment stays last");
        assert_eq!(section.rows.len(), 2);
        assert_eq!(
            section.rows[1],
            vec![
                Cell::Text("x2".into()),
                Cell::Empty,
                Cell::Text("4X-102".into()),
                Cell::Text("7".into()),
                Cell::Choice("א".into())
            ]
        );
    }

    #[test]
    fn a_repeating_section_may_be_empty_but_rows_may_not() {
        assert!(validate(&[vehicles_section()], SectionValues::new()).unwrap()[0].rows.is_empty());
        let values = SectionValues::from([("vehicles".into(), vec![record(&[("f2", text("  "))])])]);
        let errors = validate(&[vehicles_section()], values).unwrap_err();
        assert_eq!(
            codes(&errors),
            vec![
                ("sections.vehicles.0.f1", "required"),
                ("sections.vehicles.0.f3", "required"),
                ("sections.vehicles.0", "empty_row")
            ]
        );
    }

    #[test]
    fn required_fields_are_enforced_per_row() {
        let values = SectionValues::from([(
            "vehicles".into(),
            vec![
                record(&[("f1", text("x1")), ("f3", text("4X-101"))]),
                record(&[("f1", text("x2")), ("f4", text("7"))]),
            ],
        )]);
        let errors = validate(&[vehicles_section()], values).unwrap_err();
        assert_eq!(codes(&errors), vec![("sections.vehicles.1.f3", "required")]);
    }

    #[test]
    fn station_fields_accept_only_active_stations() {
        let row = |station: &str| record(&[("f1", text("a")), ("f3", text(station))]);
        let values = SectionValues::from([("links".into(), vec![row("st-1"), row("st-2"), row("st-9")])]);
        let errors = validate(&[stations_section()], values).unwrap_err();
        assert_eq!(
            codes(&errors),
            vec![("sections.links.1.f3", "station_inactive"), ("sections.links.2.f3", "unknown_station")]
        );

        let values = SectionValues::from([("links".into(), vec![row("st-1")])]);
        let cell = &validate(&[stations_section()], values).unwrap()[0].rows[0][2];
        assert_eq!(cell, &Cell::Station(NamedRef { id: "st-1".into(), name: "תחנת אורן".into() }));
    }

    #[test]
    fn single_record_sections_validate_every_field_type() {
        let values = SectionValues::from([(
            "conditions".into(),
            vec![record(&[
                ("f1", text("-12.5")),
                ("f2", FieldValue::Bool(false)),
                ("f3", FieldValue::List(vec!["ג".into(), "א".into(), "ג".into()])),
                ("f4", text("alpha")),
            ])],
        )]);
        let section = &validate(&[conditions_section()], values).unwrap()[0];
        assert_eq!(
            section.rows,
            vec![vec![
                Cell::Number("-12.5".into()),
                Cell::Boolean(false),
                Cell::Choices(vec!["ג".into(), "א".into()]),
                Cell::System(NamedRef { id: "alpha".into(), name: "מערכת אלפא".into() }),
            ]]
        );
    }

    #[test]
    fn rejects_values_that_do_not_fit_the_field() {
        let values = SectionValues::from([(
            "conditions".into(),
            vec![record(&[
                ("f1", text("12,5")),
                ("f2", text("yes")),
                ("f3", FieldValue::List(vec!["ד".into()])),
                ("f4", text("beta")),
            ])],
        )]);
        let errors = validate(&[conditions_section()], values).unwrap_err();
        assert_eq!(
            codes(&errors),
            vec![
                ("sections.conditions.0.f1", "invalid_number"),
                ("sections.conditions.0.f2", "invalid_value"),
                ("sections.conditions.0.f3", "unknown_option"),
                ("sections.conditions.0.f4", "system_not_involved"),
            ]
        );
    }

    #[test]
    fn untouched_single_sections_report_required_fields() {
        let errors = validate(&[conditions_section()], SectionValues::new()).unwrap_err();
        assert_eq!(
            codes(&errors),
            vec![("sections.conditions.0.f1", "required"), ("sections.conditions.0.f2", "required")]
        );
    }

    #[test]
    fn values_of_removed_fields_are_ignored() {
        let values = SectionValues::from([(
            "vehicles".into(),
            vec![record(&[("f1", text("x")), ("f3", text("4X")), ("gone", text("old"))])],
        )]);
        assert_eq!(validate(&[vehicles_section()], values).unwrap()[0].rows[0].len(), 5);
    }

    #[test]
    fn number_format() {
        for ok in ["0", "12", "-3", "4.25", "123456789012345"] {
            assert!(is_number(ok), "{ok}");
        }
        for bad in ["", "-", "1.", ".5", "1e3", "12,5", "+1", "1.1234567", "1234567890123456"] {
            assert!(!is_number(bad), "{bad}");
        }
    }

    // ---- administration ----

    fn field_input(id: Option<&str>, label: &str, kind: FieldKind) -> FieldInput {
        FieldInput {
            id: id.map(Into::into),
            label: label.into(),
            kind,
            required: false,
            active: true,
            options: if kind.has_options() {
                vec!["א".into(), " ".into(), "א".into()]
            } else {
                vec!["ignored".into()]
            },
        }
    }

    #[test]
    fn creates_sections_with_fields_in_the_given_order() {
        let input = SectionInput {
            id: None,
            name: " קישורים ".into(),
            mode: SectionMode::Repeating,
            fields: vec![
                field_input(None, "כותרת 1", FieldKind::Text),
                field_input(None, "תחנה", FieldKind::Station),
                field_input(None, "מקטע", FieldKind::SingleSelect),
            ],
        };
        let section = input.apply(None, "section-3".into()).unwrap();
        assert_eq!(section.id, "section-3");
        assert_eq!(section.name, "קישורים");
        assert!(section.active);
        let ids: Vec<&str> = section.fields.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["f1", "f2", "f3"]);
        assert!(section.fields[0].options.is_empty(), "options only for select fields");
        assert_eq!(section.fields[2].options, vec!["א"]);
    }

    #[test]
    fn edits_rename_reorder_add_and_remove_fields() {
        let existing = vehicles_section();
        let input = SectionInput {
            id: Some("vehicles".into()),
            name: "כלים ומקטעים".into(),
            mode: SectionMode::Repeating,
            fields: vec![
                field_input(Some("f3"), "מספר זנב", FieldKind::Text),
                field_input(Some("f1"), "כותרת 1", FieldKind::Text),
                field_input(None, "הערה", FieldKind::Text),
            ],
        };
        let section = input.apply(Some(&existing), "unused".into()).unwrap();
        let fields: Vec<(&str, &str)> = section.fields.iter().map(|f| (f.id.as_str(), f.label.as_str())).collect();
        assert_eq!(fields, vec![("f3", "מספר זנב"), ("f1", "כותרת 1"), ("f6", "הערה")]);
        assert_eq!(section.id, "vehicles");
    }

    #[test]
    fn rejects_invalid_section_definitions() {
        let input = SectionInput {
            id: None,
            name: "".into(),
            mode: SectionMode::Single,
            fields: vec![
                field_input(Some("f99"), "x", FieldKind::Text),
                field_input(None, "שם", FieldKind::Text),
                field_input(None, "שם", FieldKind::Number),
                FieldInput { options: vec![], ..field_input(None, "בחירה", FieldKind::MultiSelect) },
            ],
        };
        let errors = input.apply(None, "s".into()).unwrap_err();
        assert_eq!(
            codes(&errors),
            vec![
                ("name", "required"),
                ("fields.0", "unknown_field"),
                ("fields.2.label", "duplicate_label"),
                ("fields.3.options", "required"),
            ]
        );
        let empty = SectionInput { id: None, name: "x".into(), mode: SectionMode::Single, fields: vec![] };
        assert_eq!(codes(&empty.apply(None, "s".into()).unwrap_err()), vec![("fields", "required")]);
    }

    #[test]
    fn serialises_cells_with_explicit_types() {
        let json = serde_json::to_value(vec![Cell::Empty, Cell::Number("3".into())]).unwrap();
        assert_eq!(json, serde_json::json!([{ "type": "empty" }, { "type": "number", "value": "3" }]));
        let values: Record = serde_json::from_str(r#"{"a":"x","b":true,"c":["1"]}"#).unwrap();
        assert_eq!(values["a"], text("x"));
        assert_eq!(values["b"], FieldValue::Bool(true));
        assert_eq!(values["c"], FieldValue::List(vec!["1".into()]));
    }
}
