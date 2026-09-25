//! Stations: a centrally managed list that technical fields of type
//! `Station` refer to. Stations are renamed or deactivated, never deleted.

use serde::{Deserialize, Serialize};

use super::validation::{required_text, FieldError};

pub const MAX_STATION_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Station {
    pub id: String,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationInput {
    /// `None` creates a new station.
    pub id: Option<String>,
    pub name: String,
}

impl StationInput {
    pub fn apply(&self, existing: Option<&Station>, new_id: String) -> Result<Station, Vec<FieldError>> {
        let name =
            required_text(&self.name, MAX_STATION_NAME_CHARS).map_err(|code| vec![FieldError::new("name", code)])?;
        Ok(match existing {
            Some(station) => Station { name, ..station.clone() },
            None => Station { id: new_id, name, active: true },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_renames_stations() {
        let created = StationInput { id: None, name: " תחנת אורן ".into() }.apply(None, "station-4".into()).unwrap();
        assert_eq!(created, Station { id: "station-4".into(), name: "תחנת אורן".into(), active: true });

        let inactive = Station { active: false, ..created };
        let renamed =
            StationInput { id: Some("station-4".into()), name: "תחנת ברוש".into() }.apply(Some(&inactive), "x".into());
        assert_eq!(renamed.unwrap(), Station { id: "station-4".into(), name: "תחנת ברוש".into(), active: false });
    }

    #[test]
    fn requires_a_name() {
        let errors = StationInput { id: None, name: "  ".into() }.apply(None, "x".into()).unwrap_err();
        assert_eq!(errors, vec![FieldError::new("name", "required")]);
    }
}
