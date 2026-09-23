//! Excel adapter: turns the bytes of an operations-log .xlsx workbook into an
//! [`OperationsLog`]. The workbook is untrusted input; this module enforces
//! size limits before parsing and only ever extracts plain text.
//!
//! * `grid`    – cell values of the first worksheet (via calamine)
//! * `mapping` – header detection, column mapping, text conversion
//! * `fills`   – optional background-colour detection for pre-selection

mod fills;
mod grid;
mod mapping;

use std::io::Cursor;

pub use fills::RgbColor;

use crate::domain::operations_log::{OperationsLog, OperationsLogRow};

/// Largest workbook file accepted.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Largest total uncompressed size of the package, a guard against zip bombs.
const MAX_UNCOMPRESSED_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("workbook exceeds the size limit")]
    TooLarge,
    #[error("not a readable .xlsx workbook: {0}")]
    Unreadable(String),
    #[error("workbook has no worksheets")]
    NoWorksheet,
    #[error("required columns missing: {0:?}")]
    MissingColumns(Vec<&'static str>),
    #[error("worksheet has no data rows")]
    NoRows,
    #[error("worksheet has too many rows")]
    TooManyRows,
}

impl ImportError {
    /// Stable code reported to the UI.
    pub fn code(&self) -> &'static str {
        match self {
            ImportError::TooLarge => "file_too_large",
            ImportError::Unreadable(_) => "unreadable_workbook",
            ImportError::NoWorksheet => "no_worksheet",
            ImportError::MissingColumns(_) => "missing_columns",
            ImportError::NoRows => "no_rows",
            ImportError::TooManyRows => "too_many_rows",
        }
    }
}

/// Reads an operations log from workbook bytes. Rows with the `highlight`
/// background colour are flagged for pre-selection.
pub fn read_operations_log(bytes: &[u8], file_name: &str, highlight: &RgbColor) -> Result<OperationsLog, ImportError> {
    check_package_size(bytes)?;

    let sheet = grid::read_first_sheet(bytes).map_err(|error| match error {
        grid::GridError::NoWorksheet => ImportError::NoWorksheet,
        grid::GridError::Unreadable(detail) => ImportError::Unreadable(detail),
    })?;

    let mapped = mapping::map_rows(&sheet.rows).map_err(|error| match error {
        mapping::MappingError::MissingColumns(columns) => ImportError::MissingColumns(columns),
        mapping::MappingError::NoRows => ImportError::NoRows,
        mapping::MappingError::TooManyRows => ImportError::TooManyRows,
    })?;

    let (highlighted, highlight_detection_available) = match fills::highlighted_rows(bytes, highlight) {
        Ok(rows) => (rows, true),
        Err(error) => {
            // Not fatal: the operator can still select rows manually.
            crate::log_internal("excel highlight detection", &error);
            (Default::default(), false)
        }
    };

    let rows = mapped
        .into_iter()
        .map(|row| OperationsLogRow {
            id: row.number,
            highlighted: highlighted.contains(&row.number),
            time: row.time,
            from: row.from,
            to: row.to,
            description: row.description,
            event_type: row.event_type,
        })
        .collect();

    Ok(OperationsLog {
        source_file_name: file_name.to_owned(),
        sheet_name: sheet.name,
        rows,
        highlight_detection_available,
    })
}

/// Rejects oversized files and packages whose declared uncompressed size is
/// excessive, before any part is decompressed.
fn check_package_size(bytes: &[u8]) -> Result<(), ImportError> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ImportError::TooLarge);
    }
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| ImportError::Unreadable(error.to_string()))?;
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(|error| ImportError::Unreadable(error.to_string()))?;
        total = total.saturating_add(entry.size());
        if total > MAX_UNCOMPRESSED_BYTES {
            return Err(ImportError::TooLarge);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO_WORKBOOK: &[u8] = include_bytes!("../../../../demo/operations-log-demo.xlsx");

    fn yellow() -> RgbColor {
        RgbColor::parse("FFFF00").unwrap()
    }

    #[test]
    fn reads_the_demo_workbook() {
        let log = read_operations_log(DEMO_WORKBOOK, "operations-log-demo.xlsx", &yellow()).unwrap();
        assert_eq!(log.sheet_name, "יומן מבצעים");
        assert_eq!(log.rows.len(), 20);
        assert!(log.highlight_detection_available);

        let first = &log.rows[0];
        assert_eq!(first.id, 3, "a title row precedes the header row");
        assert_eq!(first.time, "07:00");
        assert!(!first.description.is_empty());
        assert!(!first.event_type.is_empty());
    }

    #[test]
    fn flags_rows_with_the_configured_background_colour() {
        let log = read_operations_log(DEMO_WORKBOOK, "demo.xlsx", &yellow()).unwrap();
        let highlighted: Vec<u32> = log.rows.iter().filter(|r| r.highlighted).map(|r| r.id).collect();
        assert_eq!(highlighted, vec![8, 9, 11, 13, 14, 17]);

        // A different configured colour pre-selects nothing.
        let green = RgbColor::parse("00FF00").unwrap();
        let log = read_operations_log(DEMO_WORKBOOK, "demo.xlsx", &green).unwrap();
        assert!(log.rows.iter().all(|r| !r.highlighted));
    }

    #[test]
    fn rejects_files_that_are_not_workbooks() {
        let error = read_operations_log(b"PK not really a zip", "x.xlsx", &yellow()).unwrap_err();
        assert_eq!(error.code(), "unreadable_workbook");
    }

    #[test]
    fn rejects_oversized_files_before_parsing() {
        let bytes = vec![0u8; MAX_FILE_BYTES as usize + 1];
        assert_eq!(read_operations_log(&bytes, "big.xlsx", &yellow()).unwrap_err().code(), "file_too_large");
    }
}
