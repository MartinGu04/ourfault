//! Stand-in for SharePoint. Simulates a list whose items are the editable
//! investigation forms, stored in a local JSON file. Makes no network
//! requests; the URLs it returns are fictional.

use std::path::PathBuf;
use std::sync::Mutex;

use super::{json_file, AdapterError, SharePointAdapter};
use crate::domain::investigation::{Investigation, StoredInvestigation};
use crate::domain::investigation_number::InvestigationNumber;
use crate::domain::system::SharePointDestination;

pub struct MockSharePoint {
    path: PathBuf,
    // The mutex makes create-if-absent atomic within this process, mirroring
    // the uniqueness guarantee the real backend must provide.
    records: Mutex<Vec<StoredInvestigation>>,
}

impl MockSharePoint {
    /// Opens the store at `path`, seeding it on first run.
    pub fn open(path: PathBuf, seed: impl FnOnce() -> Vec<StoredInvestigation>) -> Result<Self, AdapterError> {
        let records = json_file::load_or_seed(&path, seed)?;
        Ok(Self { path, records: Mutex::new(records) })
    }

    fn records(&self) -> Result<std::sync::MutexGuard<'_, Vec<StoredInvestigation>>, AdapterError> {
        self.records.lock().map_err(|_| AdapterError::Storage("mock SharePoint lock poisoned".into()))
    }

    /// Fictional address of the item's form in the system's configured list.
    fn item_url(destination: &SharePointDestination, item_id: u32) -> String {
        format!(
            "{}/Lists/{}/DispForm.aspx?ID={item_id}",
            destination.site_url.trim_end_matches('/'),
            destination.list.replace(' ', "%20")
        )
    }
}

impl SharePointAdapter for MockSharePoint {
    fn list_investigations(&self) -> Result<Vec<StoredInvestigation>, AdapterError> {
        Ok(self.records()?.clone())
    }

    fn find_investigation(&self, number: InvestigationNumber) -> Result<Option<StoredInvestigation>, AdapterError> {
        Ok(self.records()?.iter().find(|r| r.investigation.number == number).cloned())
    }

    fn highest_number_in_year(&self, year: u16) -> Result<Option<InvestigationNumber>, AdapterError> {
        Ok(self.records()?.iter().map(|r| r.investigation.number).filter(|n| n.year() == year).max())
    }

    fn create_investigation(
        &self,
        investigation: Investigation,
        destination: &SharePointDestination,
        created_by: &str,
        created_at: &str,
    ) -> Result<StoredInvestigation, AdapterError> {
        let mut records = self.records()?;
        let number = investigation.number;
        if records.iter().any(|r| r.investigation.number == number) {
            return Err(AdapterError::NumberTaken(number));
        }
        let item_id = records.iter().map(|r| r.item_id).max().unwrap_or(0) + 1;
        let stored = StoredInvestigation {
            investigation,
            created_at: created_at.to_owned(),
            created_by: created_by.to_owned(),
            item_id,
            url: Self::item_url(destination, item_id),
        };
        records.push(stored.clone());
        if let Err(error) = json_file::save(&self.path, &*records) {
            records.pop();
            return Err(error);
        }
        Ok(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::json_file::tests::temp_dir;
    use crate::domain::investigation::tests::{rows, system};
    use chrono::NaiveDate;

    fn investigation(sequence: u32, year: u16) -> Investigation {
        Investigation::build(
            InvestigationNumber::new(sequence, year).unwrap(),
            NaiveDate::from_ymd_opt(i32::from(year), 1, 10).unwrap(),
            Some(&system("alpha", true)),
            &rows(),
            "https://checks.example.com/runs/1",
        )
        .unwrap()
    }

    fn create(store: &MockSharePoint, sequence: u32, year: u16) -> Result<StoredInvestigation, AdapterError> {
        store.create_investigation(
            investigation(sequence, year),
            &system("alpha", true).sharepoint,
            "tester",
            "2026-01-10T09:00:00+02:00",
        )
    }

    #[test]
    fn creates_list_items_in_the_configured_list() {
        let store = MockSharePoint::open(temp_dir("sp").join("sp.json"), Vec::new).unwrap();
        let stored = create(&store, 56, 2026).unwrap();
        assert_eq!(stored.item_id, 1);
        assert_eq!(stored.url, "https://sharepoint.example.com/sites/alpha/Lists/Investigations/DispForm.aspx?ID=1");
        assert_eq!(stored.created_by, "tester");
        assert_eq!(create(&store, 57, 2026).unwrap().item_id, 2);
        let found = store.find_investigation(stored.investigation.number).unwrap();
        assert_eq!(found, Some(stored));
    }

    #[test]
    fn rejects_duplicate_numbers() {
        let store = MockSharePoint::open(temp_dir("sp").join("sp.json"), Vec::new).unwrap();
        create(&store, 56, 2026).unwrap();
        let error = create(&store, 56, 2026).unwrap_err();
        assert!(matches!(error, AdapterError::NumberTaken(n) if n.to_string() == "056-2026"));
        assert_eq!(store.list_investigations().unwrap().len(), 1);
    }

    #[test]
    fn reports_the_highest_number_per_year() {
        let store = MockSharePoint::open(temp_dir("sp").join("sp.json"), Vec::new).unwrap();
        create(&store, 412, 2025).unwrap();
        create(&store, 9, 2026).unwrap();
        create(&store, 1000, 2026).unwrap();
        assert_eq!(store.highest_number_in_year(2026).unwrap().unwrap().to_string(), "1000-2026");
        assert_eq!(store.highest_number_in_year(2025).unwrap().unwrap().to_string(), "412-2025");
        assert_eq!(store.highest_number_in_year(2027).unwrap(), None);
    }

    #[test]
    fn persists_across_restarts() {
        let path = temp_dir("sp").join("sp.json");
        {
            let store = MockSharePoint::open(path.clone(), Vec::new).unwrap();
            create(&store, 1, 2026).unwrap();
        }
        let reopened = MockSharePoint::open(path, || panic!("must not reseed")).unwrap();
        assert_eq!(reopened.list_investigations().unwrap().len(), 1);
    }
}
