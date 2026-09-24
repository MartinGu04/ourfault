//! Fictional demo data written on first run, so the application is usable
//! immediately. Nothing here refers to real systems, stations, people,
//! places or addresses.
//!
//! Past investigations are stored as draft content and built through the
//! same domain rules as real ones, so the seed can never drift from the
//! model.

use serde::Deserialize;

use crate::domain::configuration::Configuration;
use crate::domain::draft::{Draft, DraftContent};
use crate::domain::investigation::{Investigation, Publication, PublicationKind, PublishedInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::lifecycle::Lifecycle;

const CONFIGURATION_JSON: &str = include_str!("../seed/configuration.json");
const INVESTIGATIONS_JSON: &str = include_str!("../seed/investigations.json");
const DRAFTS_JSON: &str = include_str!("../seed/drafts.json");

/// Rows as Excel puts them on the clipboard (tab-separated, CRLF), copied
/// from `demo/operations-log-demo.xlsx`. Used by tests.
#[cfg(test)]
pub const DEMO_PASTE: &str = include_str!("../../demo/demo-paste.txt");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeedInvestigation {
    number: InvestigationNumber,
    item_id: u32,
    completed_at: String,
    completed_by: String,
    distributed_at: Option<String>,
    content: DraftContent,
}

pub fn configuration() -> Configuration {
    serde_json::from_str(CONFIGURATION_JSON).expect("bundled seed/configuration.json is valid")
}

pub fn drafts() -> Vec<Draft> {
    serde_json::from_str(DRAFTS_JSON).expect("bundled seed/drafts.json is valid")
}

pub fn investigations() -> Vec<PublishedInvestigation> {
    let configuration = configuration();
    // History may involve systems and stations that were deactivated since.
    let mut at_the_time = configuration.clone();
    at_the_time.systems.iter_mut().for_each(|system| system.active = true);
    at_the_time.stations.iter_mut().for_each(|station| station.active = true);

    let seeds: Vec<SeedInvestigation> =
        serde_json::from_str(INVESTIGATIONS_JSON).expect("bundled seed/investigations.json is valid");
    seeds
        .into_iter()
        .map(|seed| {
            let investigation = Investigation::build(seed.number, &seed.content, &at_the_time)
                .unwrap_or_else(|errors| panic!("seed investigation {} is invalid: {errors:?}", seed.number));
            let mut lifecycle = Lifecycle::completed(&seed.completed_at, &seed.completed_by);
            if let Some(at) = &seed.distributed_at {
                lifecycle
                    .record_distribution(at, &seed.completed_by)
                    .expect("completed investigations can be distributed");
            }
            let target = &configuration.publication;
            PublishedInvestigation {
                investigation,
                lifecycle,
                publication: Publication {
                    destination: PublicationKind::SharePoint,
                    url: format!("{}/Lists/{}/DispForm.aspx?ID={}", target.site_url, target.list, seed.item_id),
                    reference: seed.item_id.to_string(),
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::activity::{ActivityStatus, ActivityTypeKind};
    use crate::domain::lifecycle::InvestigationStatus;
    use crate::domain::sections::SectionMode;

    #[test]
    fn seed_configuration_is_valid_and_demonstrates_the_model() {
        let configuration = configuration();
        assert!(configuration.setup_status().ready);
        assert!(configuration.systems.iter().any(|s| !s.active), "a deactivated system with history");
        assert!(configuration.stations.iter().any(|s| !s.active), "a deactivated station");
        assert!(configuration.mail.validated().is_ok());
        assert!(configuration.publication.validated().is_ok());
        let modes: Vec<SectionMode> = configuration.sections.iter().map(|s| s.mode).collect();
        assert!(modes.contains(&SectionMode::Single) && modes.contains(&SectionMode::Repeating));
        let addresses = configuration.systems.iter().flat_map(|s| &s.distribution_list);
        assert!(addresses.clone().all(|a| a.ends_with("@example.com")));

        let vehicles: Vec<&str> = configuration.sections[1].fields.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(vehicles, vec!["כותרת 1", "כותרת 2", "מס׳ זנב", "מס׳ קרון", "מקטע"], "segment last");
    }

    #[test]
    fn seed_investigations_cover_every_type_and_status() {
        let investigations = investigations();
        let latest = investigations.iter().map(|p| p.investigation.number).max().unwrap();
        assert_eq!(latest.to_string(), "055-2026", "the next investigation is 056-2026");

        let kinds: Vec<ActivityTypeKind> =
            investigations.iter().map(|p| p.investigation.activity.activity_type.kind()).collect();
        for kind in [
            ActivityTypeKind::Mission,
            ActivityTypeKind::Experiment,
            ActivityTypeKind::Training,
            ActivityTypeKind::Other,
        ] {
            assert!(kinds.contains(&kind), "{kind:?}");
        }
        let statuses: Vec<InvestigationStatus> = investigations.iter().map(|p| p.lifecycle.status).collect();
        assert!(
            statuses.contains(&InvestigationStatus::Completed) && statuses.contains(&InvestigationStatus::Distributed)
        );
        assert!(investigations.iter().any(|p| p.investigation.activity.status == ActivityStatus::Active));
        assert!(investigations.iter().any(|p| p.investigation.activity.systems.len() > 2), "multi-system");
        assert!(investigations.iter().any(|p| p.investigation.sections[1].rows.len() > 1), "repeated rows");
    }

    #[test]
    fn seed_draft_is_open_and_incomplete() {
        let drafts = drafts();
        assert_eq!(drafts.len(), 1);
        assert!(drafts[0].is_open());
        assert!(crate::domain::draft::validate_draft_id(&drafts[0].id).is_ok());
        let errors =
            Investigation::build(InvestigationNumber::new(56, 2026).unwrap(), &drafts[0].content, &configuration())
                .unwrap_err();
        let fields: Vec<&str> = errors.iter().map(|e| e.field.as_str()).collect();
        assert!(fields.contains(&"rows") && fields.contains(&"preliminaryCheckUrl"));
    }
}
