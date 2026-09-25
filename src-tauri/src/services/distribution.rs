use serde::Serialize;

use super::Now;
use crate::adapters::{ConfigurationRepository, DistributionAdapter, InvestigationPublisher};
use crate::domain::investigation::PublishedInvestigation;
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::mail::{union_recipients, DistributionMessage, DistributionReceipt, MailValues};
use crate::error::AppError;
use crate::render::document::DocumentView;

/// What was sent, returned so the UI shows exactly the delivered message.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentDistribution {
    pub message: DistributionMessage,
    pub receipt: DistributionReceipt,
    /// The investigation with its new status, or `None` if the message was
    /// sent but recording the status failed (the UI says so).
    pub investigation: Option<PublishedInvestigation>,
}

pub struct DistributionService<'a> {
    publisher: &'a dyn InvestigationPublisher,
    configuration: &'a dyn ConfigurationRepository,
    distribution: &'a dyn DistributionAdapter,
}

impl<'a> DistributionService<'a> {
    pub fn new(
        publisher: &'a dyn InvestigationPublisher,
        configuration: &'a dyn ConfigurationRepository,
        distribution: &'a dyn DistributionAdapter,
    ) -> Self {
        Self { publisher, configuration, distribution }
    }

    /// Builds the message from the configured template, addressed to the
    /// union of the distribution lists of every system involved.
    pub fn compose(&self, number: InvestigationNumber) -> Result<DistributionMessage, AppError> {
        let published = self.publisher.find(number)?.ok_or(AppError::NotFound)?;
        self.message_for(&published)
    }

    fn message_for(&self, published: &PublishedInvestigation) -> Result<DistributionMessage, AppError> {
        let configuration = self.configuration.load()?.value;
        let activity = &published.investigation.activity;
        let systems = activity.systems.iter().filter_map(|system| configuration.system(&system.id));
        let values = MailValues {
            activity_name: activity.name.clone(),
            systems: activity.system_names(),
            number: published.investigation.number.to_string(),
            link: published.publication.url.clone(),
        };
        Ok(configuration.mail.render(&values, union_recipients(systems)))
    }

    /// Composes and sends the message, then records the distribution in the
    /// investigation's lifecycle. The content is always rebuilt here from
    /// stored data, never taken from the frontend.
    pub fn distribute(&self, number: InvestigationNumber, now: &Now, by: &str) -> Result<SentDistribution, AppError> {
        let published = self.publisher.find(number)?.ok_or(AppError::NotFound)?;
        let message = self.message_for(&published)?;
        if message.to.is_empty() {
            return Err(AppError::field("recipients", "no_recipients"));
        }
        let mut lifecycle = published.lifecycle.clone();
        lifecycle.record_distribution(&now.timestamp, by).map_err(|e| AppError::internal("distribution", &e))?;

        let receipt = self.distribution.send(&message, &now.timestamp)?;
        let document = DocumentView::from_investigation(&published.investigation, &lifecycle);
        let investigation = self
            .publisher
            .update_lifecycle(number, &lifecycle, &document)
            .map_err(|error| crate::log_internal("recording distribution", &error))
            .ok();
        Ok(SentDistribution { message, receipt, investigation })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::adapters::mock_distribution::MockDistribution;
    use crate::domain::lifecycle::InvestigationStatus;
    use crate::services::investigations::tests::Fixture;
    use crate::services::testing::{now, now_at};

    #[test]
    fn sends_to_every_involved_system_and_records_the_status() {
        let fixture = Fixture::new();
        let published = fixture.service().complete(&fixture.draft().id, 1, &now(), "op").unwrap().investigation;
        let mail = MockDistribution::open(temp_dir("mail").join("outbox.json")).unwrap();
        let service = DistributionService::new(&fixture.sharepoint, &fixture.base.configuration, &mail);
        let number = published.investigation.number;

        let composed = service.compose(number).unwrap();
        assert_eq!(composed.to, vec!["alpha@example.com", "beta@example.com"]);
        assert_eq!(composed.subject, "תחקיר למשימה \"בלט רומני\" - מערכת alpha, מערכת beta");
        assert!(composed
            .body_html
            .contains(&format!("<a href=\"{}\">תחקיר</a>", published.publication.url.replace('&', "&amp;"))));

        let first = service.distribute(number, &now_at("11:00"), "op").unwrap();
        let second = service.distribute(number, &now_at("12:00"), "op2").unwrap();
        assert_eq!(first.message, composed);
        assert_eq!(mail.outbox().len(), 2);

        let lifecycle = second.investigation.unwrap().lifecycle;
        assert_eq!(lifecycle.status, InvestigationStatus::Distributed);
        assert_eq!(lifecycle.distributed_at, Some(now_at("11:00").timestamp), "first distribution kept");
        assert_eq!(lifecycle.history.len(), 3);
    }

    #[test]
    fn refuses_to_send_without_recipients() {
        let fixture = Fixture::new();
        let published = fixture.service().complete(&fixture.draft().id, 1, &now(), "op").unwrap().investigation;
        let config_service = crate::services::configuration::Administration::new(
            &fixture.base.configuration,
            crate::state::AdminGrant::for_tests(),
        );
        for id in ["alpha", "beta"] {
            config_service
                .save_system(&crate::domain::system::SystemInput {
                    id: Some(id.into()),
                    name: format!("מערכת {id}"),
                    distribution_list: vec![],
                })
                .unwrap();
        }
        let mail = MockDistribution::open(temp_dir("mail").join("outbox.json")).unwrap();
        let service = DistributionService::new(&fixture.sharepoint, &fixture.base.configuration, &mail);
        let error = service.distribute(published.investigation.number, &now(), "op").unwrap_err();
        assert!(matches!(error, AppError::Validation { ref errors } if errors[0].code == "no_recipients"));
        assert!(mail.outbox().is_empty());
        let status = fixture.sharepoint.find(published.investigation.number).unwrap().unwrap().lifecycle.status;
        assert_eq!(status, InvestigationStatus::Completed);
    }

    #[test]
    fn unknown_investigations_are_not_found() {
        let fixture = Fixture::new();
        let mail = MockDistribution::open(temp_dir("mail").join("outbox.json")).unwrap();
        let service = DistributionService::new(&fixture.sharepoint, &fixture.base.configuration, &mail);
        let number = InvestigationNumber::new(5, 2026).unwrap();
        assert!(matches!(service.distribute(number, &now(), "op"), Err(AppError::NotFound)));
        assert!(mail.outbox().is_empty());
    }
}
