//! Application layer: use cases that combine domain rules with adapters.
//! Services receive their dependencies explicitly, which keeps them free of
//! Tauri and straightforward to test with in-memory adapters.

pub mod distribution;
pub mod investigations;
pub mod systems;

use chrono::{Datelike, Local, NaiveDate, SecondsFormat};

use crate::error::AppError;

/// The current moment, captured once per request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Now {
    pub date: NaiveDate,
    /// RFC 3339 local timestamp.
    pub timestamp: String,
}

impl Now {
    pub fn local() -> Self {
        let now = Local::now();
        Self { date: now.date_naive(), timestamp: now.to_rfc3339_opts(SecondsFormat::Secs, false) }
    }

    pub fn year(&self) -> Result<u16, AppError> {
        u16::try_from(self.date.year()).map_err(|e| AppError::internal("current year", &e))
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::Now;
    use chrono::NaiveDate;

    pub(crate) fn now() -> Now {
        Now { date: NaiveDate::from_ymd_opt(2026, 9, 23).unwrap(), timestamp: "2026-09-23T10:00:00+03:00".into() }
    }
}
