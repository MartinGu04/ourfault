//! Drafts: investigations in progress.
//!
//! A draft holds whatever the operator has entered so far and is saved
//! automatically. It never has an investigation number: the number is
//! allocated only when the draft is completed and published. After a
//! successful completion the draft is marked as converted (never deleted
//! first), which also prevents it from being completed twice.

use serde::{Deserialize, Serialize};

use super::activity::ActivityInput;
use super::investigation_number::InvestigationNumber;
use super::log_rows::{LogRow, MAX_ROWS};
use super::sections::{SectionValues, MAX_SECTION_ROWS};

/// Upper bound for one serialised draft. Real drafts are far smaller; the
/// limit protects storage from runaway input.
pub const MAX_DRAFT_BYTES: usize = 8 * 1024 * 1024;
const MAX_DRAFT_SECTIONS: usize = 50;
const MAX_DRAFT_ID_CHARS: usize = 64;

/// Everything the operator enters for an investigation. Untrusted and
/// possibly incomplete; completeness is checked only on completion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DraftContent {
    pub activity: ActivityInput,
    pub sections: SectionValues,
    /// Operations-log rows as pasted and reviewed (the chronology).
    pub rows: Vec<LogRow>,
    pub preliminary_check_url: String,
}

impl DraftContent {
    /// Structural limits enforced on every save. Returns an error code.
    pub fn check_limits(&self) -> Result<(), &'static str> {
        if self.rows.len() > MAX_ROWS {
            return Err("too_many_rows");
        }
        if self.sections.len() > MAX_DRAFT_SECTIONS || self.sections.values().any(|rows| rows.len() > MAX_SECTION_ROWS)
        {
            return Err("too_many_rows");
        }
        let size = serde_json::to_vec(self).map(|json| json.len()).unwrap_or(usize::MAX);
        if size > MAX_DRAFT_BYTES {
            return Err("draft_too_large");
        }
        Ok(())
    }
}

/// Where the operator was in the wizard, so a reopened draft continues there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DraftStep {
    #[default]
    Activity,
    Technical,
    Chronology,
    Review,
}

/// Record of a completed draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversion {
    pub number: InvestigationNumber,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub id: String,
    /// Incremented on every save; saves must name the revision they edited,
    /// so two workstations cannot silently overwrite each other.
    pub revision: u64,
    pub created_at: String,
    pub created_by: String,
    pub updated_at: String,
    pub step: DraftStep,
    pub content: DraftContent,
    /// Set once the draft became an investigation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converted: Option<Conversion>,
}

impl Draft {
    pub fn is_open(&self) -> bool {
        self.converted.is_none()
    }
}

/// Compact form for the drafts list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftSummary {
    pub id: String,
    pub revision: u64,
    pub activity_name: String,
    pub system_names: Vec<String>,
    pub row_count: usize,
    pub step: DraftStep,
    pub updated_at: String,
    pub created_by: String,
}

/// Draft ids become file names in the local (and future shared) store, so
/// they are restricted to a safe alphabet.
pub fn validate_draft_id(id: &str) -> Result<(), &'static str> {
    let valid = !id.is_empty()
        && id.len() <= MAX_DRAFT_ID_CHARS
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-');
    if valid {
        Ok(())
    } else {
        Err("invalid_draft_id")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_ids_cannot_escape_the_store() {
        assert!(validate_draft_id("d-20260923-1a2b3c4d5e6f7a8b").is_ok());
        for bad in ["", "../x", "..", "a/b", "a\\b", "C:x", "Draft", "-x", "d.json", &"a".repeat(65)] {
            assert!(validate_draft_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn limits_protect_storage() {
        let mut content = DraftContent::default();
        assert!(content.check_limits().is_ok());
        content.preliminary_check_url = "x".repeat(MAX_DRAFT_BYTES);
        assert_eq!(content.check_limits(), Err("draft_too_large"));
        content.preliminary_check_url.clear();
        content.rows =
            vec![LogRow { time: "1".into(), from: "a".into(), to: "b".into(), description: "c".into() }; MAX_ROWS + 1];
        assert_eq!(content.check_limits(), Err("too_many_rows"));
    }

    #[test]
    fn empty_json_is_an_empty_draft() {
        let content: DraftContent = serde_json::from_str("{}").unwrap();
        assert_eq!(content, DraftContent::default());
    }
}
