//! Operations-log rows pasted by the operator.
//!
//! The operator selects the relevant rows in Excel, copies them and pastes
//! them into OurFault. Excel puts the selection on the clipboard as
//! tab-separated text; this module turns that text into rows. Parsing is
//! purely positional and deterministic: nothing is inferred, scored or
//! filtered beyond dropping blank lines, and the operator reviews (and may
//! edit or remove) every row before continuing.
//!
//! Pasted text is untrusted input: size, row count and cell length are
//! bounded, and control characters are removed.

use serde::{Deserialize, Serialize};

/// Largest paste accepted, in bytes.
pub const MAX_PASTE_BYTES: usize = 1024 * 1024;
/// Most rows in one investigation.
pub const MAX_ROWS: usize = 500;
/// Longest accepted cell, in characters.
pub const MAX_CELL_CHARS: usize = 2000;

/// One operations-log row as it appears in an investigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRow {
    pub time: String,
    pub from: String,
    pub to: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PastedRows {
    pub rows: Vec<LogRow>,
    /// True when the first pasted line was the column-title row and was skipped.
    pub header_skipped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasteError {
    #[error("nothing to parse")]
    Empty,
    #[error("paste exceeds {MAX_PASTE_BYTES} bytes")]
    TooLarge,
    #[error("fewer than four columns")]
    TooFewColumns,
    #[error("more than {MAX_ROWS} rows")]
    TooManyRows,
    #[error("a cell exceeds {MAX_CELL_CHARS} characters")]
    CellTooLong,
}

impl PasteError {
    /// Stable code reported to the UI.
    pub fn code(self) -> &'static str {
        match self {
            PasteError::Empty => "empty_paste",
            PasteError::TooLarge => "paste_too_large",
            PasteError::TooFewColumns => "too_few_columns",
            PasteError::TooManyRows => "too_many_rows",
            PasteError::CellTooLong => "cell_too_long",
        }
    }
}

/// Column positions of the four fields in the pasted table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Columns {
    time: usize,
    from: usize,
    to: usize,
    description: usize,
}

/// The operations-log layout: Time, From, To, Description (further columns,
/// such as Event type, are not part of an investigation).
const DEFAULT_COLUMNS: Columns = Columns { time: 0, from: 1, to: 2, description: 3 };

/// Parses text copied from Excel.
///
/// If the first line holds the column titles (English or Hebrew, any order),
/// columns are taken from it and the line is skipped. Otherwise columns are
/// taken by position in the operations-log order.
pub fn parse_pasted_rows(text: &str) -> Result<PastedRows, PasteError> {
    if text.len() > MAX_PASTE_BYTES {
        return Err(PasteError::TooLarge);
    }
    let table: Vec<Vec<String>> =
        split_tab_separated(text).into_iter().filter(|row| row.iter().any(|cell| !cell.trim().is_empty())).collect();
    let first = table.first().ok_or(PasteError::Empty)?;

    let (columns, body, header_skipped) = match header_columns(first) {
        Some(columns) => (columns, &table[1..], true),
        None => (DEFAULT_COLUMNS, &table[..], false),
    };
    if body.is_empty() {
        return Err(PasteError::Empty);
    }
    let width = body.iter().map(|row| row.len()).max().unwrap_or(0);
    if !header_skipped && width < 4 {
        return Err(PasteError::TooFewColumns);
    }
    if body.len() > MAX_ROWS {
        return Err(PasteError::TooManyRows);
    }

    let rows = body
        .iter()
        .map(|cells| {
            let cell = |index: usize| cells.get(index).map(|value| clean_cell(value)).unwrap_or_default();
            let row = LogRow {
                time: cell(columns.time),
                from: cell(columns.from),
                to: cell(columns.to),
                description: cell(columns.description),
            };
            if row_fields(&row).iter().any(|field| field.chars().count() > MAX_CELL_CHARS) {
                Err(PasteError::CellTooLong)
            } else {
                Ok(row)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PastedRows { rows, header_skipped })
}

/// Validates rows as submitted from the review step (possibly edited by the
/// operator) and returns them cleaned.
pub fn validate_rows(rows: &[LogRow]) -> Result<Vec<LogRow>, &'static str> {
    if rows.is_empty() {
        return Err("no_rows");
    }
    if rows.len() > MAX_ROWS {
        return Err("too_many_rows");
    }
    rows.iter()
        .map(|row| {
            let cleaned = LogRow {
                time: clean_cell(&row.time),
                from: clean_cell(&row.from),
                to: clean_cell(&row.to),
                description: clean_cell(&row.description),
            };
            let fields = row_fields(&cleaned);
            if fields.iter().any(|field| field.chars().count() > MAX_CELL_CHARS) {
                Err("row_too_long")
            } else if fields.iter().all(|field| field.is_empty()) {
                Err("empty_row")
            } else {
                Ok(cleaned)
            }
        })
        .collect()
}

fn row_fields(row: &LogRow) -> [&str; 4] {
    [&row.time, &row.from, &row.to, &row.description]
}

/// Trims, normalises line endings and removes control characters other than
/// line breaks and tabs. The UI always renders the result as plain text.
fn clean_cell(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .trim()
        .to_owned()
}

fn header_columns(row: &[String]) -> Option<Columns> {
    let find = |aliases: &[&str]| row.iter().position(|cell| aliases.contains(&normalise_title(cell).as_str()));
    Some(Columns {
        time: find(&["time", "שעה", "זמן"])?,
        from: find(&["from", "ממי", "מאת"])?,
        to: find(&["to", "למי", "אל"])?,
        description: find(&["description", "תוכן", "תיאור"])?,
    })
}

fn normalise_title(text: &str) -> String {
    text.trim().trim_end_matches(':').split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Splits Excel clipboard text into rows of cells. Rows end with CRLF (or
/// LF/CR); cells are separated by tabs. Excel wraps a cell in double quotes
/// when it contains a line break, doubling any quotes inside it. A cell that
/// merely starts with a quote but is not a well-formed quoted field is taken
/// literally.
fn split_tab_separated(text: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut position = 0;
    loop {
        let (cell, end) = read_cell(&chars, position);
        row.push(cell);
        match chars.get(end) {
            Some('\t') => position = end + 1,
            Some('\r') if chars.get(end + 1) == Some(&'\n') => {
                rows.push(std::mem::take(&mut row));
                position = end + 2;
            }
            Some(_) => {
                rows.push(std::mem::take(&mut row));
                position = end + 1;
            }
            None => {
                rows.push(row);
                return rows;
            }
        }
    }
}

fn is_separator(c: char) -> bool {
    matches!(c, '\t' | '\r' | '\n')
}

/// Reads one cell starting at `start`; returns its value and the index of the
/// separator (or end of input) that follows it.
fn read_cell(chars: &[char], start: usize) -> (String, usize) {
    if chars.get(start) == Some(&'"') {
        if let Some(quoted) = read_quoted_cell(chars, start) {
            return quoted;
        }
    }
    let end = chars[start..].iter().position(|c| is_separator(*c)).map_or(chars.len(), |offset| start + offset);
    (chars[start..end].iter().collect(), end)
}

fn read_quoted_cell(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut value = String::new();
    let mut index = start + 1;
    while index < chars.len() {
        if chars[index] == '"' {
            if chars.get(index + 1) == Some(&'"') {
                value.push('"');
                index += 2;
                continue;
            }
            // A closing quote must be followed by a separator or the end.
            return match chars.get(index + 1) {
                None => Some((value, index + 1)),
                Some(c) if is_separator(*c) => Some((value, index + 1)),
                Some(_) => None,
            };
        }
        value.push(chars[index]);
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(time: &str, from: &str, to: &str, description: &str) -> LogRow {
        LogRow { time: time.into(), from: from.into(), to: to.into(), description: description.into() }
    }

    #[test]
    fn parses_rows_copied_from_excel() {
        // Excel: CRLF between rows, trailing CRLF, event type as fifth column.
        let text =
            "08:14\tעמדה 3\tמוקד מבצעים\tזוהתה האטה\tתקלה\r\n08:16\tמוקד מבצעים\tתורן טכני\tדווח לתורן\tתקלה\r\n";
        let pasted = parse_pasted_rows(text).unwrap();
        assert!(!pasted.header_skipped);
        assert_eq!(
            pasted.rows,
            vec![
                row("08:14", "עמדה 3", "מוקד מבצעים", "זוהתה האטה"),
                row("08:16", "מוקד מבצעים", "תורן טכני", "דווח לתורן"),
            ]
        );
    }

    #[test]
    fn keeps_multi_line_cells_and_quotes() {
        let text = "09:05\tא\tב\t\"שורה ראשונה\nשורה \"\"שנייה\"\"\"\r\n09:10\tא\tב\t\"ציטוט\" בתחילת תא\r\n";
        let pasted = parse_pasted_rows(text).unwrap();
        assert_eq!(pasted.rows[0].description, "שורה ראשונה\nשורה \"שנייה\"");
        assert_eq!(pasted.rows[1].description, "\"ציטוט\" בתחילת תא", "not a quoted field, taken literally");
    }

    #[test]
    fn uses_a_pasted_header_row_to_locate_columns() {
        let text = "תוכן\tלמי\tממי\tשעה\r\nשיחה\tב\tא\t09:00\r\n";
        let pasted = parse_pasted_rows(text).unwrap();
        assert!(pasted.header_skipped);
        assert_eq!(pasted.rows, vec![row("09:00", "א", "ב", "שיחה")]);

        let english = parse_pasted_rows("Time\tFrom\tTo\tDescription\tEvent type\n10:00\ta\tb\tc\td").unwrap();
        assert!(english.header_skipped);
        assert_eq!(english.rows, vec![row("10:00", "a", "b", "c")]);
    }

    #[test]
    fn keeps_empty_cells_in_position_and_drops_blank_lines() {
        let text = "\r\n10:00\t\tב\tתוכן\r\n\t\t\t\r\n10:05\tא\t\t\r\n";
        let pasted = parse_pasted_rows(text).unwrap();
        assert_eq!(pasted.rows, vec![row("10:00", "", "ב", "תוכן"), row("10:05", "א", "", "")]);
    }

    #[test]
    fn rejects_input_that_does_not_look_like_log_rows() {
        assert_eq!(parse_pasted_rows(""), Err(PasteError::Empty));
        assert_eq!(parse_pasted_rows(" \r\n\t\r\n"), Err(PasteError::Empty));
        assert_eq!(parse_pasted_rows("just some text"), Err(PasteError::TooFewColumns));
        assert_eq!(parse_pasted_rows("08:00\tא\tב"), Err(PasteError::TooFewColumns));
        assert_eq!(parse_pasted_rows("Time\tFrom\tTo\tDescription\r\n"), Err(PasteError::Empty));
    }

    #[test]
    fn enforces_limits() {
        assert_eq!(parse_pasted_rows(&"x".repeat(MAX_PASTE_BYTES + 1)), Err(PasteError::TooLarge));
        let many = "08:00\ta\tb\tc\n".repeat(MAX_ROWS + 1);
        assert_eq!(parse_pasted_rows(&many), Err(PasteError::TooManyRows));
        let long = format!("08:00\ta\tb\t{}", "x".repeat(MAX_CELL_CHARS + 1));
        assert_eq!(parse_pasted_rows(&long), Err(PasteError::CellTooLong));
    }

    #[test]
    fn removes_control_characters_and_keeps_markup_literal() {
        let pasted = parse_pasted_rows("10:00\ta\u{0}b\t=1+1\t<img src=x onerror=alert(1)>").unwrap();
        assert_eq!(pasted.rows[0], row("10:00", "ab", "=1+1", "<img src=x onerror=alert(1)>"));
    }

    #[test]
    fn validates_reviewed_rows() {
        assert_eq!(validate_rows(&[]), Err("no_rows"));
        assert_eq!(validate_rows(&[row(" ", "", "", "\u{7}")]), Err("empty_row"));
        assert_eq!(validate_rows(&[row("08:00", "a", "b", &"x".repeat(MAX_CELL_CHARS + 1))]), Err("row_too_long"));
        assert_eq!(validate_rows(&vec![row("08:00", "a", "b", "c"); MAX_ROWS + 1]), Err("too_many_rows"));
        assert_eq!(validate_rows(&[row(" 08:00 ", "a", "b", "c\r\nd")]), Ok(vec![row("08:00", "a", "b", "c\nd")]));
    }
}
