//! Investigation status, separate from the activity status.
//!
//! ```text
//! Draft ──complete──▶ Completed ──distribute──▶ Distributed ──distribute again──▶ Distributed
//! ```
//!
//! A draft has no number and lives in the draft repository. Completion
//! allocates the number and publishes it. Nothing ever moves backwards, and
//! every transition is appended to the history, so later actions never erase
//! when an investigation was completed or first distributed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvestigationStatus {
    Draft,
    Completed,
    Distributed,
}

impl InvestigationStatus {
    /// Whether `self → next` is an allowed transition.
    pub fn can_become(self, next: InvestigationStatus) -> bool {
        use InvestigationStatus::*;
        matches!((self, next), (Draft, Completed) | (Completed, Distributed) | (Distributed, Distributed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("cannot change investigation status from {from:?} to {to:?}")]
pub struct TransitionError {
    pub from: InvestigationStatus,
    pub to: InvestigationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    pub status: InvestigationStatus,
    /// RFC 3339 local timestamp.
    pub at: String,
    pub by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lifecycle {
    pub status: InvestigationStatus,
    pub completed_at: String,
    pub completed_by: String,
    /// First distribution. Kept when the investigation is distributed again.
    pub distributed_at: Option<String>,
    /// Every transition, oldest first. Append-only.
    pub history: Vec<StatusEvent>,
}

impl Lifecycle {
    /// The lifecycle of a draft that has just been completed.
    pub fn completed(at: &str, by: &str) -> Self {
        Self {
            status: InvestigationStatus::Completed,
            completed_at: at.to_owned(),
            completed_by: by.to_owned(),
            distributed_at: None,
            history: vec![StatusEvent { status: InvestigationStatus::Completed, at: at.to_owned(), by: by.to_owned() }],
        }
    }

    pub fn record_distribution(&mut self, at: &str, by: &str) -> Result<(), TransitionError> {
        let to = InvestigationStatus::Distributed;
        if !self.status.can_become(to) {
            return Err(TransitionError { from: self.status, to });
        }
        self.status = to;
        self.distributed_at.get_or_insert_with(|| at.to_owned());
        self.history.push(StatusEvent { status: to, at: at.to_owned(), by: by.to_owned() });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::InvestigationStatus::*;
    use super::*;

    #[test]
    fn only_forward_transitions_are_allowed() {
        let allowed = [(Draft, Completed), (Completed, Distributed), (Distributed, Distributed)];
        for from in [Draft, Completed, Distributed] {
            for to in [Draft, Completed, Distributed] {
                assert_eq!(from.can_become(to), allowed.contains(&(from, to)), "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn distribution_keeps_the_completion_and_first_distribution() {
        let mut lifecycle = Lifecycle::completed("2026-09-23T10:00:00+03:00", "op1");
        assert_eq!(lifecycle.status, Completed);

        lifecycle.record_distribution("2026-09-23T11:00:00+03:00", "op1").unwrap();
        lifecycle.record_distribution("2026-09-24T09:00:00+03:00", "op2").unwrap();

        assert_eq!(lifecycle.status, Distributed);
        assert_eq!(lifecycle.completed_at, "2026-09-23T10:00:00+03:00");
        assert_eq!(lifecycle.distributed_at.as_deref(), Some("2026-09-23T11:00:00+03:00"));
        let statuses: Vec<_> = lifecycle.history.iter().map(|event| (event.status, event.by.as_str())).collect();
        assert_eq!(statuses, vec![(Completed, "op1"), (Distributed, "op1"), (Distributed, "op2")]);
    }

    #[test]
    fn a_draft_lifecycle_cannot_be_distributed() {
        let mut lifecycle = Lifecycle { status: Draft, ..Lifecycle::completed("t", "op") };
        assert_eq!(lifecycle.record_distribution("t2", "op"), Err(TransitionError { from: Draft, to: Distributed }));
        assert_eq!(lifecycle.distributed_at, None);
    }
}
