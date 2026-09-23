//! Reads the first worksheet into a plain grid of cell values using calamine.
//! calamine only reads cached values: formulas are not evaluated and macros
//! are never loaded or executed.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};

/// A cell value, independent of the parsing library.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Empty,
    Text(String),
    Number(f64),
    DateTime(DateTimeParts),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTimeParts {
    /// `None` for time-only values.
    pub date: Option<(u16, u8, u8)>,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawRow {
    /// 1-based worksheet row number.
    pub number: u32,
    pub cells: Vec<Cell>,
}

#[derive(Debug)]
pub struct RawSheet {
    pub name: String,
    pub rows: Vec<RawRow>,
}

#[derive(Debug, thiserror::Error)]
pub enum GridError {
    #[error("workbook could not be parsed: {0}")]
    Unreadable(String),
    #[error("workbook has no worksheets")]
    NoWorksheet,
}

pub fn read_first_sheet(bytes: &[u8]) -> Result<RawSheet, GridError> {
    let mut workbook = Xlsx::new(Cursor::new(bytes)).map_err(|e| GridError::Unreadable(e.to_string()))?;
    let name = workbook.sheet_names().first().cloned().ok_or(GridError::NoWorksheet)?;
    let range = workbook.worksheet_range(&name).map_err(|e| GridError::Unreadable(e.to_string()))?;
    let (first_row, first_col) = range.start().unwrap_or((0, 0));

    let rows = range
        .rows()
        .enumerate()
        .map(|(index, cells)| {
            // Pad so that column indexes are absolute (A = 0) even when the
            // used range does not start in column A.
            let mut values = vec![Cell::Empty; first_col as usize];
            values.extend(cells.iter().map(convert));
            RawRow { number: first_row + index as u32 + 1, cells: values }
        })
        .collect();

    Ok(RawSheet { name, rows })
}

fn convert(data: &Data) -> Cell {
    match data {
        Data::Empty | Data::Error(_) => Cell::Empty,
        Data::String(text) | Data::DateTimeIso(text) | Data::DurationIso(text) => Cell::Text(text.clone()),
        Data::Bool(value) => Cell::Text(value.to_string()),
        Data::Int(value) => Cell::Number(*value as f64),
        Data::Float(value) => Cell::Number(*value),
        Data::DateTime(value) => {
            let (year, month, day, hour, minute, second, _millis) = value.to_ymd_hms_milli();
            let time_only = value.as_f64() < 1.0;
            Cell::DateTime(DateTimeParts { date: (!time_only).then_some((year, month, day)), hour, minute, second })
        }
    }
}
