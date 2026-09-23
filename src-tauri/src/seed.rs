//! Fictional demo data written on first run, so the application is usable
//! immediately. Nothing here refers to real systems, people or addresses.

use crate::domain::investigation::StoredInvestigation;
use crate::domain::system::System;

const SYSTEMS_JSON: &str = include_str!("../seed/systems.json");
const INVESTIGATIONS_JSON: &str = include_str!("../seed/investigations.json");

/// The bundled demo operations log (see `scripts/generate_demo_workbook.py`).
pub const DEMO_WORKBOOK: &[u8] = include_bytes!("../../demo/operations-log-demo.xlsx");
pub const DEMO_WORKBOOK_NAME: &str = "operations-log-demo.xlsx";

pub fn systems() -> Vec<System> {
    serde_json::from_str(SYSTEMS_JSON).expect("bundled seed/systems.json is valid")
}

pub fn investigations() -> Vec<StoredInvestigation> {
    serde_json::from_str(INVESTIGATIONS_JSON).expect("bundled seed/investigations.json is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::system::SystemInput;

    #[test]
    fn seed_data_is_valid_and_consistent() {
        let systems = systems();
        assert!(systems.iter().any(|s| s.active));
        assert!(systems.iter().any(|s| !s.active), "demo includes a deactivated system with history");
        for system in &systems {
            // Seeds must satisfy the same rules as administrator input.
            let input = SystemInput {
                id: Some(system.id.clone()),
                name: system.name.clone(),
                active: system.active,
                template: system.template.clone(),
                distribution_list: system.distribution_list.clone(),
                sharepoint: system.sharepoint.clone(),
            };
            assert_eq!(&input.validate(system.id.clone()).unwrap(), system);
            assert!(system.distribution_list.iter().all(|a| a.ends_with("@example.com")));
        }

        let investigations = investigations();
        for stored in &investigations {
            assert!(systems.iter().any(|s| s.id == stored.investigation.system.id));
        }
        let latest = investigations.iter().map(|s| s.investigation.number).max().unwrap();
        assert_eq!(latest.to_string(), "055-2026");
    }
}
