//! Findings about a draft, in three severities:
//!
//! * `Error` – blocks completion (missing required values, invalid values,
//!   an end before its start). These are the validation errors of
//!   [`crate::domain::investigation::Investigation::build`].
//! * `Warning` – does not block, but deserves the operator's attention.
//! * `Info` – does not block; points out a meaningful difference.
//!
//! Deterministic rules only. Codes are language-neutral message keys; the UI
//! words them. Not a rule engine: each rule is a few lines below.

use chrono::Duration;
use serde::Serialize;

use super::activity::{ActivityInput, LocalDateTime};
use super::night_window::NightWindow;
use super::validation::FieldError;

/// Differences between planned and actual times up to this size are normal
/// scheduling noise and are not pointed out.
pub const PLAN_DEVIATION_TOLERANCE_MINUTES: i64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Advisory {
    pub severity: Severity,
    /// Stable message key, e.g. `night_overlap_not_marked` or `required`.
    pub code: &'static str,
    /// The input the finding is about (same keys as validation errors).
    pub field: String,
}

pub const NIGHT_OVERLAP_NOT_MARKED: &str = "night_overlap_not_marked";
pub const ACTUAL_STARTED_BEFORE_PLAN: &str = "actual_started_before_plan";
pub const ACTUAL_STARTED_AFTER_PLAN: &str = "actual_started_after_plan";
pub const ACTUAL_ENDED_AFTER_PLAN: &str = "actual_ended_after_plan";

impl Advisory {
    fn new(severity: Severity, code: &'static str, field: &str) -> Self {
        Self { severity, code, field: field.to_owned() }
    }
}

impl From<FieldError> for Advisory {
    fn from(error: FieldError) -> Self {
        Self { severity: Severity::Error, code: error.code, field: error.field }
    }
}

/// A complete, well-ordered period, or `None`.
fn period(start: &str, end: &str) -> Option<(LocalDateTime, LocalDateTime)> {
    let (start, end) = (start.parse::<LocalDateTime>().ok()?, end.parse::<LocalDateTime>().ok()?);
    (end >= start).then_some((start, end))
}

/// Non-blocking findings (warnings and information) about the activity.
/// Values that are missing or invalid are skipped here; they are errors.
pub fn assess_activity(activity: &ActivityInput, night: &NightWindow) -> Vec<Advisory> {
    let mut advisories = Vec::new();
    let planned = period(&activity.planned_start, &activity.planned_end);
    let actual = period(&activity.actual_start, &activity.actual_end);

    // Never changes the answer: the operator decides what a night activity is.
    if activity.night_activity == Some(false) {
        let at_night = [planned, actual].into_iter().flatten().any(|(s, e)| night.overlaps(s.naive(), e.naive()));
        if at_night {
            advisories.push(Advisory::new(Severity::Warning, NIGHT_OVERLAP_NOT_MARKED, "nightActivity"));
        }
    }

    // Planned and actual times may legitimately differ; point it out only.
    let tolerance = Duration::minutes(PLAN_DEVIATION_TOLERANCE_MINUTES);
    let planned_start = activity.planned_start.parse::<LocalDateTime>().ok();
    let planned_end = activity.planned_end.parse::<LocalDateTime>().ok();
    let actual_start = activity.actual_start.parse::<LocalDateTime>().ok();
    let actual_end = activity.actual_end.parse::<LocalDateTime>().ok();
    if let (Some(planned), Some(actual)) = (planned_start, actual_start) {
        if actual.naive() < planned.naive() - tolerance {
            advisories.push(Advisory::new(Severity::Info, ACTUAL_STARTED_BEFORE_PLAN, "actualStart"));
        } else if actual.naive() > planned.naive() + tolerance {
            advisories.push(Advisory::new(Severity::Info, ACTUAL_STARTED_AFTER_PLAN, "actualStart"));
        }
    }
    if let (Some(planned), Some(actual)) = (planned_end, actual_end) {
        if actual.naive() > planned.naive() + tolerance {
            advisories.push(Advisory::new(Severity::Info, ACTUAL_ENDED_AFTER_PLAN, "actualEnd"));
        }
    }
    advisories
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::activity::tests::activity_input;

    fn codes(activity: &ActivityInput) -> Vec<(Severity, &'static str)> {
        assess_activity(activity, &NightWindow::default()).into_iter().map(|a| (a.severity, a.code)).collect()
    }

    fn daytime() -> ActivityInput {
        ActivityInput {
            planned_start: "2026-09-20T08:00".into(),
            planned_end: "2026-09-20T12:00".into(),
            actual_start: "2026-09-20T08:10".into(),
            actual_end: "2026-09-20T12:05".into(),
            night_activity: Some(false),
            ..activity_input()
        }
    }

    #[test]
    fn a_daytime_activity_on_schedule_has_no_findings() {
        assert_eq!(codes(&daytime()), vec![]);
    }

    #[test]
    fn warns_when_a_night_period_is_not_marked_as_night_activity() {
        let mut activity = daytime();
        activity.actual_end = "2026-09-20T21:00".into();
        assert!(codes(&activity).contains(&(Severity::Warning, NIGHT_OVERLAP_NOT_MARKED)));

        // Crossing midnight in the plan, entirely at night, or over several days.
        for (start, end) in [
            ("2026-09-20T23:00", "2026-09-21T01:00"),
            ("2026-09-21T01:00", "2026-09-21T04:00"),
            ("2026-09-20T08:00", "2026-09-23T17:00"),
        ] {
            let activity = ActivityInput {
                planned_start: start.into(),
                planned_end: end.into(),
                actual_start: String::new(),
                actual_end: String::new(),
                ..daytime()
            };
            assert_eq!(codes(&activity), vec![(Severity::Warning, NIGHT_OVERLAP_NOT_MARKED)], "{start} → {end}");
        }
    }

    #[test]
    fn no_night_warning_when_marked_or_unanswered() {
        let mut activity = daytime();
        activity.planned_end = "2026-09-21T02:00".into();
        activity.night_activity = Some(true);
        assert!(!codes(&activity).iter().any(|(_, code)| *code == NIGHT_OVERLAP_NOT_MARKED));
        activity.night_activity = None;
        assert!(!codes(&activity).iter().any(|(_, code)| *code == NIGHT_OVERLAP_NOT_MARKED));
    }

    #[test]
    fn never_changes_the_operators_answer() {
        let mut activity = daytime();
        activity.planned_end = "2026-09-21T02:00".into();
        let before = activity.clone();
        assess_activity(&activity, &NightWindow::default());
        assert_eq!(activity, before);
    }

    #[test]
    fn an_actual_start_before_the_plan_is_information_only() {
        // The manual-test case: planned 15/09 02:43, actual 14/09 02:45.
        let activity = ActivityInput {
            planned_start: "2026-09-15T02:43".into(),
            planned_end: "2026-09-15T06:00".into(),
            actual_start: "2026-09-14T02:45".into(),
            actual_end: "2026-09-14T05:00".into(),
            night_activity: Some(true),
            ..daytime()
        };
        assert_eq!(codes(&activity), vec![(Severity::Info, ACTUAL_STARTED_BEFORE_PLAN)]);
    }

    #[test]
    fn late_starts_and_ends_are_information_beyond_the_tolerance() {
        let mut activity = daytime();
        activity.actual_start = "2026-09-20T08:15".into(); // exactly the tolerance
        activity.actual_end = "2026-09-20T12:15".into();
        assert_eq!(codes(&activity), vec![]);

        activity.actual_start = "2026-09-20T08:16".into();
        activity.actual_end = "2026-09-20T12:16".into();
        assert_eq!(
            codes(&activity),
            vec![(Severity::Info, ACTUAL_STARTED_AFTER_PLAN), (Severity::Info, ACTUAL_ENDED_AFTER_PLAN)]
        );
    }

    #[test]
    fn missing_or_invalid_times_produce_no_advisories() {
        let activity = ActivityInput {
            planned_start: "tomorrow".into(),
            planned_end: String::new(),
            actual_start: "2026-09-20T10:00".into(),
            actual_end: "2026-09-20T09:00".into(), // reversed: an error, not a night finding
            ..daytime()
        };
        assert_eq!(codes(&activity), vec![]);
    }

    #[test]
    fn validation_errors_become_error_advisories() {
        let advisory = Advisory::from(FieldError::new("plannedEnd", "end_before_start"));
        assert_eq!(advisory, Advisory::new(Severity::Error, "end_before_start", "plannedEnd"));
        let json = serde_json::to_value(&advisory).unwrap();
        assert_eq!(json, serde_json::json!({ "severity": "error", "code": "end_before_start", "field": "plannedEnd" }));
    }
}
