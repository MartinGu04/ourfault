//! Stand-in for the mail system. Appends messages to a local "outbox" JSON
//! file instead of sending them.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::{json_file, AdapterError, DistributionAdapter};
use crate::domain::distribution::{DistributionMessage, DistributionReceipt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxEntry {
    pub receipt: DistributionReceipt,
    pub message: DistributionMessage,
}

pub struct MockDistribution {
    path: PathBuf,
    outbox: Mutex<Vec<OutboxEntry>>,
}

impl MockDistribution {
    pub fn open(path: PathBuf) -> Result<Self, AdapterError> {
        let outbox = json_file::load_or_seed(&path, Vec::new)?;
        Ok(Self { path, outbox: Mutex::new(outbox) })
    }

    #[cfg(test)]
    pub fn outbox(&self) -> Vec<OutboxEntry> {
        self.outbox.lock().map(|entries| entries.clone()).unwrap_or_default()
    }
}

impl DistributionAdapter for MockDistribution {
    fn send(&self, message: &DistributionMessage, sent_at: &str) -> Result<DistributionReceipt, AdapterError> {
        if message.to.is_empty() {
            return Err(AdapterError::Storage("distribution message has no recipients".into()));
        }
        let mut outbox = self.outbox.lock().map_err(|_| AdapterError::Storage("outbox lock poisoned".into()))?;
        let receipt = DistributionReceipt {
            message_id: format!("mock-{:05}", outbox.len() + 1),
            sent_at: sent_at.to_owned(),
            recipient_count: message.to.len(),
        };
        outbox.push(OutboxEntry { receipt: receipt.clone(), message: message.clone() });
        if let Err(error) = json_file::save(&self.path, &*outbox) {
            outbox.pop();
            return Err(error);
        }
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;

    fn message(to: &[&str]) -> DistributionMessage {
        DistributionMessage {
            to: to.iter().map(|s| s.to_string()).collect(),
            subject: "תחקיר 056-2026".into(),
            body: "גוף ההודעה".into(),
        }
    }

    #[test]
    fn records_sent_messages_in_the_outbox() {
        let path = temp_dir("mail").join("outbox.json");
        let mail = MockDistribution::open(path.clone()).unwrap();

        let first = mail.send(&message(&["a@example.com", "b@example.com"]), "2026-09-23T10:00:00+03:00").unwrap();
        let second = mail.send(&message(&["a@example.com"]), "2026-09-23T10:05:00+03:00").unwrap();

        assert_eq!(first.message_id, "mock-00001");
        assert_eq!(first.recipient_count, 2);
        assert_eq!(second.message_id, "mock-00002");
        let reopened = MockDistribution::open(path).unwrap();
        assert_eq!(reopened.outbox().len(), 2);
        assert_eq!(reopened.outbox()[0].message.to, vec!["a@example.com", "b@example.com"]);
    }

    #[test]
    fn refuses_messages_without_recipients() {
        let mail = MockDistribution::open(temp_dir("mail").join("outbox.json")).unwrap();
        assert!(mail.send(&message(&[]), "2026-09-23T10:00:00+03:00").is_err());
        assert!(mail.outbox().is_empty());
    }
}
