use serde::{Deserialize, Serialize};

use super::investigation::StoredInvestigation;

/// A message ready to hand to a distribution adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistributionMessage {
    pub to: Vec<String>,
    pub subject: String,
    /// Plain text. Never interpreted as HTML by OurFault.
    pub body: String,
}

/// Confirmation returned by a distribution adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistributionReceipt {
    pub message_id: String,
    /// RFC 3339 local timestamp.
    pub sent_at: String,
    pub recipient_count: usize,
}

impl DistributionMessage {
    /// Composes the notification sent to a system's distribution list when an
    /// investigation is created. The message links to the investigation in
    /// SharePoint, which is where it is read and edited, instead of embedding
    /// its content.
    pub fn for_investigation(stored: &StoredInvestigation, recipients: &[String]) -> Self {
        let inv = &stored.investigation;
        let subject = format!("{} {} – {}", inv.template.title, inv.number, inv.system.name);
        let body = [
            "שלום,".to_owned(),
            String::new(),
            format!("נוצר תחקיר חדש במערכת {}.", inv.system.name),
            String::new(),
            format!("מספר תחקיר: {}", inv.number),
            format!("תאריך: {}", inv.date.format("%d/%m/%Y")),
            format!("תבנית: {}", inv.template.name),
            format!("שורות מיומן המבצעים: {}", inv.rows.len()),
            format!("בדיקות מקדימות: {}", inv.preliminary_check_url),
            String::new(),
            format!("לצפייה ולעריכת התחקיר ב-SharePoint:\n{}", stored.url),
            String::new(),
            "הודעה זו נשלחה מ-OurFault.".to_owned(),
        ]
        .join("\n");
        Self { to: recipients.to_vec(), subject, body }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::investigation::tests::{rows, system};
    use crate::domain::investigation::Investigation;
    use crate::domain::investigation_number::InvestigationNumber;
    use chrono::NaiveDate;

    #[test]
    fn composes_subject_and_body_from_the_investigation() {
        let alpha = system("alpha", true);
        let investigation = Investigation::build(
            InvestigationNumber::new(56, 2026).unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 23).unwrap(),
            Some(&alpha),
            &rows(),
            "https://checks.example.com/runs/1",
        )
        .unwrap();
        let stored = StoredInvestigation {
            investigation,
            created_at: "2026-09-23T10:00:00+03:00".into(),
            created_by: "tester".into(),
            item_id: 57,
            url: "https://sharepoint.example.com/sites/alpha/Lists/Investigations/DispForm.aspx?ID=57".into(),
        };

        let message = DistributionMessage::for_investigation(&stored, &alpha.distribution_list);

        assert_eq!(message.to, vec!["alpha@example.com"]);
        assert_eq!(message.subject, "תחקיר אירוע 056-2026 – מערכת alpha");
        assert!(message.body.contains("תאריך: 23/09/2026"));
        assert!(message.body.contains("שורות מיומן המבצעים: 2"));
        assert!(message.body.contains(&stored.url));
    }
}
