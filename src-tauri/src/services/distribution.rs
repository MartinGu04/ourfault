use serde::Serialize;

use super::Now;
use crate::adapters::{DistributionAdapter, SharePointAdapter, SystemRepository};
use crate::domain::distribution::{DistributionMessage, DistributionReceipt};
use crate::domain::investigation_number::InvestigationNumber;
use crate::error::AppError;

/// What was sent, returned so the UI shows exactly the delivered message.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentDistribution {
    pub message: DistributionMessage,
    pub receipt: DistributionReceipt,
}

pub struct DistributionService<'a> {
    sharepoint: &'a dyn SharePointAdapter,
    systems: &'a dyn SystemRepository,
    distribution: &'a dyn DistributionAdapter,
}

impl<'a> DistributionService<'a> {
    pub fn new(
        sharepoint: &'a dyn SharePointAdapter,
        systems: &'a dyn SystemRepository,
        distribution: &'a dyn DistributionAdapter,
    ) -> Self {
        Self { sharepoint, systems, distribution }
    }

    /// Builds the message for an investigation, addressed to the distribution
    /// list currently configured for its system.
    pub fn compose(&self, number: InvestigationNumber) -> Result<DistributionMessage, AppError> {
        let stored = self.sharepoint.find_investigation(number)?.ok_or(AppError::NotFound)?;
        let system = self.systems.get(&stored.investigation.system.id)?.ok_or(AppError::NotFound)?;
        Ok(DistributionMessage::for_investigation(&stored, &system.distribution_list))
    }

    /// Composes and sends the message. The content is always rebuilt here from
    /// stored data, never taken from the frontend.
    pub fn distribute(&self, number: InvestigationNumber, now: &Now) -> Result<SentDistribution, AppError> {
        let message = self.compose(number)?;
        let receipt = self.distribution.send(&message, &now.timestamp)?;
        Ok(SentDistribution { message, receipt })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::json_system_repository::JsonSystemRepository;
    use crate::adapters::mock_distribution::MockDistribution;
    use crate::adapters::mock_sharepoint::MockSharePoint;
    use crate::domain::investigation::tests::{log, system};
    use crate::domain::investigation::InvestigationDraft;
    use crate::services::investigations::InvestigationService;
    use crate::services::testing::now;

    #[test]
    fn sends_to_the_system_distribution_list() {
        let dir = temp_dir("distribution");
        let sharepoint = MockSharePoint::open(dir.join("sp.json"), Vec::new).unwrap();
        let systems = JsonSystemRepository::open(dir.join("systems.json"), || vec![system("alpha", true)]).unwrap();
        let mail = MockDistribution::open(dir.join("outbox.json")).unwrap();
        let draft = InvestigationDraft {
            import_id: 1,
            selected_row_ids: vec![2],
            system_id: "alpha".into(),
            preliminary_check_url: "https://checks.example.com/1".into(),
        };
        let stored = InvestigationService::new(&sharepoint, &systems).create(&draft, &log(), &now(), "op").unwrap();
        let service = DistributionService::new(&sharepoint, &systems, &mail);

        let sent = service.distribute(stored.investigation.number, &now()).unwrap();

        assert_eq!(sent.message.to, vec!["alpha@example.com"]);
        assert_eq!(sent.receipt.recipient_count, 1);
        assert_eq!(mail.outbox().len(), 1);
        assert_eq!(mail.outbox()[0].message, sent.message);
    }

    #[test]
    fn unknown_investigations_are_not_found() {
        let dir = temp_dir("distribution");
        let sharepoint = MockSharePoint::open(dir.join("sp.json"), Vec::new).unwrap();
        let systems = JsonSystemRepository::open(dir.join("systems.json"), Vec::new).unwrap();
        let mail = MockDistribution::open(dir.join("outbox.json")).unwrap();
        let service = DistributionService::new(&sharepoint, &systems, &mail);
        let number = InvestigationNumber::new(5, 2026).unwrap();
        assert!(matches!(service.distribute(number, &now()), Err(AppError::NotFound)));
        assert!(mail.outbox().is_empty());
    }
}
