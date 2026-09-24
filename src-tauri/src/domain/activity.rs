//! The activity an investigation belongs to: a mission, experiment, training
//! or other activity, carried out with one or more systems.
//!
//! [`ActivityInput`] is what the operator entered (possibly incomplete, as
//! saved in a draft). [`ActivityInput::validate`] turns it into an
//! [`Activity`], applying every rule and reporting all violations at once.

use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::configuration::Configuration;
use super::validation::{required_text, FieldError};

pub const MAX_ACTIVITY_NAME_CHARS: usize = 120;
pub const MAX_OTHER_TYPE_CHARS: usize = 80;
pub const MAX_SYSTEMS: usize = 20;

/// Activity type as chosen in the form. Labels are a UI concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityTypeKind {
    Mission,
    Experiment,
    Training,
    Other,
}

/// Validated activity type. `Other` always carries its description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ActivityType {
    Mission,
    Experiment,
    Training,
    Other { description: String },
}

impl ActivityType {
    pub fn kind(&self) -> ActivityTypeKind {
        match self {
            ActivityType::Mission => ActivityTypeKind::Mission,
            ActivityType::Experiment => ActivityTypeKind::Experiment,
            ActivityType::Training => ActivityTypeKind::Training,
            ActivityType::Other { .. } => ActivityTypeKind::Other,
        }
    }
}

/// Status of the activity itself, independent of the investigation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityStatus {
    Active,
    Completed,
}

/// Local wall-clock date and time with minute precision, exchanged as
/// `YYYY-MM-DDTHH:MM` (the format of an HTML `datetime-local` input).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDateTime(NaiveDateTime);

impl LocalDateTime {
    const FORMAT: &'static str = "%Y-%m-%dT%H:%M";

    pub fn date(self) -> NaiveDate {
        self.0.date()
    }

    pub fn naive(self) -> NaiveDateTime {
        self.0
    }
}

impl fmt::Display for LocalDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format(Self::FORMAT))
    }
}

impl FromStr for LocalDateTime {
    type Err = &'static str;

    /// Accepts `YYYY-MM-DDTHH:MM`, optionally with seconds (which are dropped).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let parsed = NaiveDateTime::parse_from_str(s, Self::FORMAT)
            .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
            .map_err(|_| "invalid_datetime")?;
        if !(2000..=2100).contains(&parsed.year()) {
            return Err("invalid_datetime");
        }
        let minutes = parsed.format(Self::FORMAT).to_string();
        NaiveDateTime::parse_from_str(&minutes, Self::FORMAT).map(Self).map_err(|_| "invalid_datetime")
    }
}

impl Serialize for LocalDateTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for LocalDateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

/// Snapshot of a system as it was when the investigation was created, so that
/// renaming a system later does not rewrite history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemRef {
    pub id: String,
    pub name: String,
}

/// What the operator entered about the activity. Every field may be empty
/// while the investigation is a draft.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ActivityInput {
    pub name: String,
    pub activity_type: Option<ActivityTypeKind>,
    /// Required when `activity_type` is `Other`; ignored otherwise.
    pub activity_type_other: String,
    pub system_ids: Vec<String>,
    pub status: Option<ActivityStatus>,
    pub planned_start: String,
    pub planned_end: String,
    pub actual_start: String,
    pub actual_end: String,
    /// Yes/no questions must be answered explicitly, so `None` is "not answered".
    pub night_activity: Option<bool>,
    pub senior_staffing: Option<bool>,
}

/// Planned times. Both are required.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedTimes {
    pub start: LocalDateTime,
    pub end: LocalDateTime,
}

/// Actual times. The end stays empty while the activity is still active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActualTimes {
    pub start: LocalDateTime,
    pub end: Option<LocalDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub name: String,
    pub activity_type: ActivityType,
    /// One or more systems, in the order the operator chose them. There is
    /// no "primary" system.
    pub systems: Vec<SystemRef>,
    pub status: ActivityStatus,
    pub planned: PlannedTimes,
    pub actual: ActualTimes,
    pub night_activity: bool,
    pub senior_staffing: bool,
}

impl Activity {
    /// The date the investigation is filed under: the actual start.
    pub fn date(&self) -> NaiveDate {
        self.actual.start.date()
    }

    pub fn system_names(&self) -> Vec<String> {
        self.systems.iter().map(|system| system.name.clone()).collect()
    }
}

impl ActivityInput {
    /// Validates the input against the current configuration. Violations are
    /// appended to `errors`; `None` is returned if there were any.
    pub fn validate(&self, configuration: &Configuration, errors: &mut Vec<FieldError>) -> Option<Activity> {
        let before = errors.len();
        let mut check = |field: &str, result: Result<String, &'static str>| {
            result.unwrap_or_else(|code| {
                errors.push(FieldError::new(field, code));
                String::new()
            })
        };

        let name = check("activityName", required_text(&self.name, MAX_ACTIVITY_NAME_CHARS));
        let other = match self.activity_type {
            Some(ActivityTypeKind::Other) => {
                check("activityTypeOther", required_text(&self.activity_type_other, MAX_OTHER_TYPE_CHARS))
            }
            _ => String::new(),
        };
        let activity_type = match self.activity_type {
            None => {
                errors.push(FieldError::new("activityType", "required"));
                None
            }
            Some(ActivityTypeKind::Mission) => Some(ActivityType::Mission),
            Some(ActivityTypeKind::Experiment) => Some(ActivityType::Experiment),
            Some(ActivityTypeKind::Training) => Some(ActivityType::Training),
            Some(ActivityTypeKind::Other) => Some(ActivityType::Other { description: other }),
        };

        let systems = self.validate_systems(configuration, errors);

        if self.status.is_none() {
            errors.push(FieldError::new("activityStatus", "required"));
        }

        let planned_start = required_time(&self.planned_start, "plannedStart", errors);
        let planned_end = required_time(&self.planned_end, "plannedEnd", errors);
        let actual_start = required_time(&self.actual_start, "actualStart", errors);
        let actual_end = match (self.status, self.actual_end.trim().is_empty()) {
            // The actual end may stay empty while the activity is still active.
            (Some(ActivityStatus::Active) | None, true) => None,
            _ => required_time(&self.actual_end, "actualEnd", errors),
        };
        check_order(planned_start, planned_end, "plannedEnd", errors);
        check_order(actual_start, actual_end, "actualEnd", errors);

        if self.night_activity.is_none() {
            errors.push(FieldError::new("nightActivity", "required"));
        }
        if self.senior_staffing.is_none() {
            errors.push(FieldError::new("seniorStaffing", "required"));
        }

        if errors.len() > before {
            return None;
        }
        Some(Activity {
            name,
            activity_type: activity_type?,
            systems,
            status: self.status?,
            planned: PlannedTimes { start: planned_start?, end: planned_end? },
            actual: ActualTimes { start: actual_start?, end: actual_end },
            night_activity: self.night_activity?,
            senior_staffing: self.senior_staffing?,
        })
    }

    /// At least one system; every system must exist and be active. Duplicates
    /// are dropped, the operator's order is kept.
    fn validate_systems(&self, configuration: &Configuration, errors: &mut Vec<FieldError>) -> Vec<SystemRef> {
        let mut systems: Vec<SystemRef> = Vec::new();
        for id in &self.system_ids {
            if systems.iter().any(|system| &system.id == id) {
                continue;
            }
            match configuration.system(id) {
                None => {
                    errors.push(FieldError::new("systemIds", "unknown_system"));
                    return Vec::new();
                }
                Some(system) if !system.active => {
                    errors.push(FieldError::new("systemIds", "system_inactive"));
                    return Vec::new();
                }
                Some(system) => systems.push(SystemRef { id: system.id.clone(), name: system.name.clone() }),
            }
        }
        if systems.is_empty() {
            errors.push(FieldError::new("systemIds", "required"));
        } else if systems.len() > MAX_SYSTEMS {
            errors.push(FieldError::new("systemIds", "too_many"));
        }
        systems
    }
}

fn required_time(value: &str, field: &str, errors: &mut Vec<FieldError>) -> Option<LocalDateTime> {
    if value.trim().is_empty() {
        errors.push(FieldError::new(field, "required"));
        return None;
    }
    value.parse().map_err(|code| errors.push(FieldError::new(field, code))).ok()
}

/// An end cannot be earlier than its corresponding start.
fn check_order(start: Option<LocalDateTime>, end: Option<LocalDateTime>, field: &str, errors: &mut Vec<FieldError>) {
    if let (Some(start), Some(end)) = (start, end) {
        if end < start {
            errors.push(FieldError::new(field, "end_before_start"));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::domain::configuration::tests::configuration;

    pub(crate) fn activity_input() -> ActivityInput {
        ActivityInput {
            name: "  בלט רומני ".into(),
            activity_type: Some(ActivityTypeKind::Mission),
            activity_type_other: "ignored unless Other".into(),
            system_ids: vec!["alpha".into(), "beta".into()],
            status: Some(ActivityStatus::Completed),
            planned_start: "2026-09-20T08:00".into(),
            planned_end: "2026-09-20T12:00".into(),
            actual_start: "2026-09-20T08:15".into(),
            actual_end: "2026-09-20T12:40".into(),
            night_activity: Some(false),
            senior_staffing: Some(true),
        }
    }

    fn validate(input: &ActivityInput) -> Result<Activity, Vec<FieldError>> {
        let mut errors = Vec::new();
        input.validate(&configuration(), &mut errors).ok_or(errors)
    }

    fn codes(errors: &[FieldError]) -> Vec<(&str, &str)> {
        errors.iter().map(|e| (e.field.as_str(), e.code)).collect()
    }

    #[test]
    fn valid_input_becomes_a_multi_system_activity() {
        let activity = validate(&activity_input()).unwrap();
        assert_eq!(activity.name, "בלט רומני");
        assert_eq!(activity.activity_type, ActivityType::Mission);
        assert_eq!(activity.system_names(), vec!["מערכת alpha", "מערכת beta"]);
        assert_eq!(activity.actual.end.unwrap().to_string(), "2026-09-20T12:40");
        assert_eq!(activity.date().to_string(), "2026-09-20");
    }

    #[test]
    fn keeps_the_operator_order_and_drops_duplicate_systems() {
        let mut input = activity_input();
        input.system_ids = vec!["beta".into(), "alpha".into(), "beta".into()];
        let ids: Vec<String> = validate(&input).unwrap().systems.into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["beta", "alpha"]);
    }

    #[test]
    fn requires_at_least_one_known_active_system() {
        let mut input = activity_input();
        input.system_ids = vec![];
        assert_eq!(codes(&validate(&input).unwrap_err()), vec![("systemIds", "required")]);
        input.system_ids = vec!["alpha".into(), "retired".into()];
        assert_eq!(codes(&validate(&input).unwrap_err()), vec![("systemIds", "system_inactive")]);
        input.system_ids = vec!["nope".into()];
        assert_eq!(codes(&validate(&input).unwrap_err()), vec![("systemIds", "unknown_system")]);
    }

    #[test]
    fn activity_type_is_required() {
        let mut input = activity_input();
        input.activity_type = None;
        assert_eq!(codes(&validate(&input).unwrap_err()), vec![("activityType", "required")]);
        for (kind, expected) in [
            (ActivityTypeKind::Experiment, ActivityType::Experiment),
            (ActivityTypeKind::Training, ActivityType::Training),
        ] {
            input.activity_type = Some(kind);
            assert_eq!(validate(&input).unwrap().activity_type, expected);
        }
    }

    #[test]
    fn other_type_requires_a_description() {
        let mut input = activity_input();
        input.activity_type = Some(ActivityTypeKind::Other);
        input.activity_type_other = "  ".into();
        assert_eq!(codes(&validate(&input).unwrap_err()), vec![("activityTypeOther", "required")]);

        input.activity_type_other = " תרגיל משולב ".into();
        let activity = validate(&input).unwrap();
        assert_eq!(activity.activity_type, ActivityType::Other { description: "תרגיל משולב".into() });
        assert_eq!(activity.activity_type.kind(), ActivityTypeKind::Other);
    }

    #[test]
    fn ends_cannot_precede_their_starts() {
        let mut input = activity_input();
        input.planned_end = "2026-09-20T07:59".into();
        input.actual_end = "2026-09-19T23:00".into();
        assert_eq!(
            codes(&validate(&input).unwrap_err()),
            vec![("plannedEnd", "end_before_start"), ("actualEnd", "end_before_start")]
        );
    }

    #[test]
    fn planned_and_actual_times_do_not_need_to_match() {
        let mut input = activity_input();
        input.planned_start = "2026-09-18T20:00".into();
        input.planned_end = "2026-09-18T23:00".into();
        input.actual_start = "2026-09-20T02:00".into();
        input.actual_end = "2026-09-20T02:00".into();
        assert!(validate(&input).is_ok(), "a later, zero-length actual window is still valid");
    }

    #[test]
    fn an_active_activity_may_have_no_actual_end() {
        let mut input = activity_input();
        input.status = Some(ActivityStatus::Active);
        input.actual_end = String::new();
        let activity = validate(&input).unwrap();
        assert_eq!(activity.status, ActivityStatus::Active);
        assert_eq!(activity.actual.end, None);

        input.status = Some(ActivityStatus::Completed);
        assert_eq!(codes(&validate(&input).unwrap_err()), vec![("actualEnd", "required")]);
    }

    #[test]
    fn reports_every_missing_field_of_an_empty_draft() {
        let errors = validate(&ActivityInput::default()).unwrap_err();
        let fields: Vec<&str> = errors.iter().map(|e| e.field.as_str()).collect();
        assert_eq!(
            fields,
            vec![
                "activityName",
                "activityType",
                "systemIds",
                "activityStatus",
                "plannedStart",
                "plannedEnd",
                "actualStart",
                "nightActivity",
                "seniorStaffing"
            ]
        );
    }

    #[test]
    fn parses_datetime_local_values() {
        assert_eq!("2026-09-20T08:15".parse::<LocalDateTime>().unwrap().to_string(), "2026-09-20T08:15");
        assert_eq!("2026-09-20T08:15:59".parse::<LocalDateTime>().unwrap().to_string(), "2026-09-20T08:15");
        for bad in ["", "20/09/2026 08:15", "2026-13-01T00:00", "1999-01-01T00:00", "2026-09-20"] {
            assert_eq!(bad.parse::<LocalDateTime>(), Err("invalid_datetime"), "{bad}");
        }
        let mut input = activity_input();
        input.actual_start = "yesterday".into();
        assert_eq!(codes(&validate(&input).unwrap_err()), vec![("actualStart", "invalid_datetime")]);
    }

    #[test]
    fn serialises_types_without_display_text() {
        let json = serde_json::to_value(ActivityType::Other { description: "x".into() }).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "other", "description": "x" }));
        assert_eq!(serde_json::to_value(ActivityType::Mission).unwrap(), serde_json::json!({ "kind": "mission" }));
        assert_eq!(serde_json::to_value(ActivityStatus::Active).unwrap(), "active");
    }
}
