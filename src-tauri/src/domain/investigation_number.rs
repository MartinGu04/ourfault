use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Identifier of an investigation, displayed as `{running number}-{year}`,
/// for example `056-2026`.
///
/// * The running number restarts at 1 every calendar year.
/// * It is displayed with a minimum width of three digits (`001`, `056`, `999`)
///   but is not limited to three digits (`1000-2026` is valid).
///
/// Ordering is chronological: by year, then by running number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InvestigationNumber {
    // Field order matters for the derived `Ord`.
    year: u16,
    sequence: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NumberError {
    #[error("invalid investigation number format")]
    InvalidFormat,
    #[error("running number must be at least 1")]
    ZeroSequence,
    #[error("year out of range")]
    YearOutOfRange,
    #[error("running number overflow")]
    Overflow,
}

impl InvestigationNumber {
    pub const MIN_YEAR: u16 = 2000;
    pub const MAX_YEAR: u16 = 9999;

    pub fn new(sequence: u32, year: u16) -> Result<Self, NumberError> {
        if sequence == 0 {
            return Err(NumberError::ZeroSequence);
        }
        if !(Self::MIN_YEAR..=Self::MAX_YEAR).contains(&year) {
            return Err(NumberError::YearOutOfRange);
        }
        Ok(Self { year, sequence })
    }

    pub fn year(self) -> u16 {
        self.year
    }

    /// The number that follows `highest` within `year`, or `001-{year}` when the
    /// year has no investigations yet. A `highest` from another year is ignored.
    pub fn next_after(highest: Option<InvestigationNumber>, year: u16) -> Result<Self, NumberError> {
        match highest {
            Some(current) if current.year == year => {
                let sequence = current.sequence.checked_add(1).ok_or(NumberError::Overflow)?;
                Self::new(sequence, year)
            }
            _ => Self::new(1, year),
        }
    }

    /// Parses user search input. Accepts the canonical form (`056-2026`), an
    /// unpadded form (`56-2026`) and a bare running number (`56`), which is
    /// interpreted in `default_year`.
    pub fn parse_lenient(input: &str, default_year: u16) -> Result<Self, NumberError> {
        let trimmed = input.trim();
        if !trimmed.contains('-') {
            return Self::new(parse_digits(trimmed)?, default_year);
        }
        trimmed.parse()
    }
}

fn parse_digits<T: FromStr>(part: &str) -> Result<T, NumberError> {
    // `u32::from_str` accepts a leading '+', which is not a valid investigation number.
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(NumberError::InvalidFormat);
    }
    part.parse().map_err(|_| NumberError::InvalidFormat)
}

impl fmt::Display for InvestigationNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:03}-{}", self.sequence, self.year)
    }
}

impl FromStr for InvestigationNumber {
    type Err = NumberError;

    /// Strict parse of `{running number}-{four-digit year}`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (sequence, year) = s.trim().split_once('-').ok_or(NumberError::InvalidFormat)?;
        if year.len() != 4 {
            return Err(NumberError::InvalidFormat);
        }
        Self::new(parse_digits(sequence)?, parse_digits(year)?)
    }
}

impl Serialize for InvestigationNumber {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for InvestigationNumber {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(sequence: u32, year: u16) -> InvestigationNumber {
        InvestigationNumber::new(sequence, year).unwrap()
    }

    #[test]
    fn formats_with_minimum_three_digits() {
        assert_eq!(num(1, 2026).to_string(), "001-2026");
        assert_eq!(num(56, 2026).to_string(), "056-2026");
        assert_eq!(num(999, 2026).to_string(), "999-2026");
    }

    #[test]
    fn formats_beyond_three_digits_without_truncation() {
        assert_eq!(num(1000, 2026).to_string(), "1000-2026");
        assert_eq!(num(123_456, 2027).to_string(), "123456-2027");
    }

    #[test]
    fn increments_within_the_same_year() {
        let next = InvestigationNumber::next_after(Some(num(55, 2026)), 2026).unwrap();
        assert_eq!(next, num(56, 2026));
        let next = InvestigationNumber::next_after(Some(num(999, 2026)), 2026).unwrap();
        assert_eq!(next.to_string(), "1000-2026");
    }

    #[test]
    fn restarts_for_a_new_year() {
        assert_eq!(InvestigationNumber::next_after(None, 2026).unwrap(), num(1, 2026));
        let next = InvestigationNumber::next_after(Some(num(412, 2025)), 2026).unwrap();
        assert_eq!(next, num(1, 2026));
    }

    #[test]
    fn increment_overflow_is_an_error() {
        let highest = num(u32::MAX, 2026);
        assert_eq!(InvestigationNumber::next_after(Some(highest), 2026), Err(NumberError::Overflow));
    }

    #[test]
    fn parses_canonical_and_unpadded_forms() {
        assert_eq!("056-2026".parse::<InvestigationNumber>().unwrap(), num(56, 2026));
        assert_eq!("56-2026".parse::<InvestigationNumber>().unwrap(), num(56, 2026));
        assert_eq!(" 1000-2026 ".parse::<InvestigationNumber>().unwrap(), num(1000, 2026));
    }

    #[test]
    fn rejects_malformed_numbers() {
        for input in ["", "-2026", "56-", "56-26", "000-2026", "+5-2026", "5a-2026", "56/2026", "56-1999", "56-2026-1"]
        {
            assert!(input.parse::<InvestigationNumber>().is_err(), "should reject {input:?}");
        }
    }

    #[test]
    fn lenient_parse_uses_default_year_for_bare_numbers() {
        assert_eq!(InvestigationNumber::parse_lenient("56", 2026).unwrap(), num(56, 2026));
        assert_eq!(InvestigationNumber::parse_lenient("12-2025", 2026).unwrap(), num(12, 2025));
        assert!(InvestigationNumber::parse_lenient("abc", 2026).is_err());
    }

    #[test]
    fn orders_by_year_then_sequence() {
        assert!(num(999, 2025) < num(1, 2026));
        assert!(num(56, 2026) < num(1000, 2026));
    }

    #[test]
    fn round_trips_through_json_as_a_string() {
        let json = serde_json::to_string(&num(56, 2026)).unwrap();
        assert_eq!(json, "\"056-2026\"");
        assert_eq!(serde_json::from_str::<InvestigationNumber>(&json).unwrap(), num(56, 2026));
        assert!(serde_json::from_str::<InvestigationNumber>("\"x\"").is_err());
    }
}
