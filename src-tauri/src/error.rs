//! Errors as seen by the frontend. Only stable codes cross the IPC boundary;
//! technical details (paths, library messages) are logged, never returned.

use serde::Serialize;

use crate::adapters::AdapterError;
use crate::domain::log_rows::PasteError;
use crate::domain::validation::FieldError;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AppError {
    /// One or more inputs are invalid.
    Validation {
        errors: Vec<FieldError>,
    },
    /// The pasted text could not be turned into rows.
    Paste {
        code: &'static str,
    },
    NotFound,
    /// The item changed in the meantime (`changed`) or can no longer be
    /// changed (`draft_converted`). Nothing was written.
    Conflict {
        code: &'static str,
    },
    /// A shared destination (SharePoint, network folder) is unreachable.
    /// Nothing was published and no number was allocated.
    Unavailable,
    /// The current work mode does not allow this action.
    Forbidden,
    /// Anything unexpected. Details are in the log only.
    Internal,
}

impl AppError {
    pub fn validation(errors: Vec<FieldError>) -> Self {
        AppError::Validation { errors }
    }

    pub fn field(field: &str, code: &'static str) -> Self {
        AppError::Validation { errors: vec![FieldError::new(field, code)] }
    }

    /// Logs `error` and returns the generic internal error.
    pub fn internal(context: &str, error: &dyn std::fmt::Display) -> Self {
        crate::log_internal(context, error);
        AppError::Internal
    }
}

impl From<AdapterError> for AppError {
    fn from(error: AdapterError) -> Self {
        match error {
            AdapterError::Conflict => AppError::Conflict { code: "changed" },
            AdapterError::Unavailable(_) => {
                crate::log_internal("destination unavailable", &error);
                AppError::Unavailable
            }
            AdapterError::NumberTaken(_) | AdapterError::DraftAlreadyPublished | AdapterError::Storage(_) => {
                AppError::internal("adapter", &error)
            }
        }
    }
}

impl From<PasteError> for AppError {
    fn from(error: PasteError) -> Self {
        AppError::Paste { code: error.code() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialises_to_stable_codes_without_details() {
        let storage = AppError::from(AdapterError::Storage("C:\\Users\\secret\\file.json: denied".into()));
        assert_eq!(serde_json::to_value(storage).unwrap(), json!({ "kind": "internal" }));

        let offline = AppError::from(AdapterError::Unavailable("\\\\server\\share unreachable".into()));
        assert_eq!(serde_json::to_value(offline).unwrap(), json!({ "kind": "unavailable" }));

        let conflict = AppError::from(AdapterError::Conflict);
        assert_eq!(serde_json::to_value(conflict).unwrap(), json!({ "kind": "conflict", "code": "changed" }));

        let paste = AppError::from(PasteError::TooFewColumns);
        assert_eq!(serde_json::to_value(paste).unwrap(), json!({ "kind": "paste", "code": "too_few_columns" }));

        let validation = AppError::field("preliminaryCheckUrl", "invalid_url");
        assert_eq!(
            serde_json::to_value(validation).unwrap(),
            json!({ "kind": "validation", "errors": [{ "field": "preliminaryCheckUrl", "code": "invalid_url" }] })
        );
    }
}
