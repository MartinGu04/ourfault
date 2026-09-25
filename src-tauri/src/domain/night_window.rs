//! The hours that count as night, used to point out activities that overlap
//! them. Stored in the configuration (default 20:00 → 06:00) so an
//! administrator can change it later.
//!
//! OurFault never decides whether an activity *is* a night activity: that
//! answer stays with the operator. The window only feeds an advisory.

use std::fmt;
use std::str::FromStr;

use chrono::{Duration, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A time of day with minute precision, exchanged as `HH:MM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeOfDay(NaiveTime);

impl TimeOfDay {
    pub fn hm(hour: u32, minute: u32) -> Self {
        Self(NaiveTime::from_hms_opt(hour, minute, 0).expect("valid time of day"))
    }
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format("%H:%M"))
    }
}

impl FromStr for TimeOfDay {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        NaiveTime::parse_from_str(s.trim(), "%H:%M").map(Self).map_err(|_| "invalid_time")
    }
}

impl Serialize for TimeOfDay {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TimeOfDay {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

/// Night hours, from `start` to `end`. When `end` is earlier than `start`
/// (the usual case, e.g. 20:00 → 06:00) the window runs past midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NightWindow {
    pub start: TimeOfDay,
    pub end: TimeOfDay,
}

impl Default for NightWindow {
    fn default() -> Self {
        Self { start: TimeOfDay::hm(20, 0), end: TimeOfDay::hm(6, 0) }
    }
}

impl NightWindow {
    /// Whether the period `[start, end)` shares any time with night hours
    /// (night hours are also half-open, so an activity ending exactly at
    /// 20:00 or starting exactly at 06:00 does not overlap them). A
    /// zero-length period overlaps if that moment is at night. Periods of any
    /// length and across any number of calendar days are handled; a reversed
    /// period never overlaps (it is reported as an error elsewhere).
    pub fn overlaps(&self, start: NaiveDateTime, end: NaiveDateTime) -> bool {
        if end < start || self.start == self.end {
            return false;
        }
        // A full day always contains the whole window.
        if end - start >= Duration::days(1) {
            return true;
        }
        // Night segments that can touch the period begin at the earliest on
        // the day before it starts.
        let mut day = start.date() - Duration::days(1);
        while day <= end.date() {
            let segment_start = day.and_time(self.start.0);
            let segment_end = if self.start < self.end {
                day.and_time(self.end.0)
            } else {
                (day + Duration::days(1)).and_time(self.end.0)
            };
            let hit = if start == end {
                segment_start <= start && start < segment_end
            } else {
                segment_start < end && start < segment_end
            };
            if hit {
                return true;
            }
            day += Duration::days(1);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M").unwrap()
    }

    fn overlaps(start: &str, end: &str) -> bool {
        NightWindow::default().overlaps(at(start), at(end))
    }

    #[test]
    fn daytime_only_activities_do_not_overlap() {
        assert!(!overlaps("2026-09-20T08:00", "2026-09-20T17:00"));
        assert!(!overlaps("2026-09-20T06:00", "2026-09-20T20:00"), "window boundaries are exclusive");
    }

    #[test]
    fn activities_touching_the_evening_or_morning_overlap() {
        assert!(overlaps("2026-09-20T18:00", "2026-09-20T20:30"));
        assert!(overlaps("2026-09-20T05:30", "2026-09-20T09:00"));
    }

    #[test]
    fn activities_crossing_midnight_overlap() {
        assert!(overlaps("2026-09-20T23:00", "2026-09-21T01:00"));
    }

    #[test]
    fn activities_entirely_at_night_overlap() {
        assert!(overlaps("2026-09-21T01:00", "2026-09-21T04:00"));
        assert!(overlaps("2026-09-20T21:00", "2026-09-20T23:00"));
        assert!(overlaps("2026-09-21T03:00", "2026-09-21T03:00"), "a single moment at night");
        assert!(!overlaps("2026-09-21T12:00", "2026-09-21T12:00"));
    }

    #[test]
    fn multi_day_activities_overlap() {
        assert!(overlaps("2026-09-20T08:00", "2026-09-22T17:00"));
        assert!(overlaps("2026-09-20T08:00", "2026-09-21T07:00"), "less than a day, but through the night");
    }

    #[test]
    fn reversed_periods_and_empty_windows_never_overlap() {
        assert!(!overlaps("2026-09-21T03:00", "2026-09-20T03:00"));
        let empty = NightWindow { start: TimeOfDay::hm(20, 0), end: TimeOfDay::hm(20, 0) };
        assert!(!empty.overlaps(at("2026-09-20T00:00"), at("2026-09-23T00:00")));
    }

    #[test]
    fn a_window_within_one_day_is_supported() {
        let window = NightWindow { start: TimeOfDay::hm(0, 0), end: TimeOfDay::hm(5, 0) };
        assert!(window.overlaps(at("2026-09-20T23:00"), at("2026-09-21T00:30")));
        assert!(!window.overlaps(at("2026-09-20T05:00"), at("2026-09-20T23:59")));
    }

    #[test]
    fn serialises_as_hours_and_minutes() {
        let json = serde_json::to_value(NightWindow::default()).unwrap();
        assert_eq!(json, serde_json::json!({ "start": "20:00", "end": "06:00" }));
        assert!(serde_json::from_str::<TimeOfDay>("\"25:00\"").is_err());
    }
}
