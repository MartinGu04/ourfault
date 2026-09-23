use serde::Serialize;

/// One row of the operations-log workbook, after mapping and sanitising.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationsLogRow {
    /// 1-based worksheet row number. Stable identifier for selection.
    pub id: u32,
    pub time: String,
    pub from: String,
    pub to: String,
    pub description: String,
    /// Present in the log for context; not copied into investigations.
    pub event_type: String,
    /// True when the row carries the configured background colour in Excel.
    /// Only used to pre-select rows; the operator's selection is authoritative.
    pub highlighted: bool,
}

/// An imported operations log, held in memory for the current session only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationsLog {
    pub source_file_name: String,
    pub sheet_name: String,
    pub rows: Vec<OperationsLogRow>,
    /// False when background colours could not be read. Rows are then simply
    /// not pre-selected; import itself still succeeds.
    pub highlight_detection_available: bool,
}
