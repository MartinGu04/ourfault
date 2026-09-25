//! Operations-log rows pasted by the operator.
//!
//! The operator selects the relevant rows in Excel, copies them and pastes
//! them into OurFault. Excel puts the selection on the clipboard as
//! tab-separated text; this module turns that text into rows. Parsing is
//! purely positional and deterministic: nothing is inferred, scored or
//! filtered beyond dropping blank lines, and the operator reviews (and may
//! edit or remove) every row before continuing.
//!
//! Pasted text is untrusted input and is only ever treated as data: it is
//! never interpreted as markup or code, here or in any renderer. Only the
//! plain-text clipboard flavour reaches this module (the UI never reads the
//! HTML flavour). Size, rows, columns and cell length are bounded; control
//! characters and invisible bidirectional overrides are removed, while
//! Hebrew, Latin, punctuation and line breaks inside cells are kept as is.
//!
//! Limits (see docs/ARCHITECTURE.md): generous for a real chronology (a busy
//! day of log rows with long descriptions fits several times over), small
//! enough to keep the review table, drafts and PDFs responsive.

use serde::{Deserialize, Serialize};

/// Largest paste accepted, in bytes (2 MiB).
pub const MAX_PASTE_BYTES: usize = 2 * 1024 * 1024;
/// Most rows in one paste, and in one investigation.
pub const MAX_ROWS: usize = 2000;
/// Most columns in a pasted row. The log has a handful; a selection of whole
/// sheet rows in Excel stays well below this.
pub const MAX_COLUMNS: usize = 64;
/// Longest accepted cell, in characters.
pub const MAX_CELL_CHARS: usize = 4000;
/// Columns an operations-log row needs: time, from, to, description.
const REQUIRED_COLUMNS: usize = 4;

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
    #[error("more than {MAX_COLUMNS} columns")]
    TooManyColumns,
    #[error("rows do not share the log's column layout")]
    NotTabular,
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
            PasteError::TooManyColumns => "too_many_columns",
            PasteError::NotTabular => "not_tabular",
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
    if table.iter().any(|row| row.len() > MAX_COLUMNS) {
        return Err(PasteError::TooManyColumns);
    }
    let width = body.iter().map(|row| row.len()).max().unwrap_or(0);
    if !header_skipped && width < REQUIRED_COLUMNS {
        return Err(PasteError::TooFewColumns);
    }
    // Excel copies a rectangular selection: every row carries all the log
    // columns (empty cells included). A shorter row means the text is not
    // a copy of log rows, e.g. prose with a stray tab.
    let needed = [columns.time, columns.from, columns.to, columns.description].into_iter().max().unwrap_or(0) + 1;
    if body.iter().any(|row| row.len() < needed) {
        return Err(PasteError::NotTabular);
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
/// line breaks and tabs, plus invisible characters that could make text
/// display differently from what it contains (bidirectional embeddings,
/// overrides and isolates, byte-order marks). Everything else, including
/// Hebrew, directional marks and markup-like text, is kept literally; every
/// renderer treats it as plain text.
fn clean_cell(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| (!c.is_control() || *c == '\n' || *c == '\t') && !is_hidden_formatting(*c))
        .collect::<String>()
        .trim()
        .to_owned()
}

fn is_hidden_formatting(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
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
    fn rejects_unrelated_text() {
        let prose = "שלום לכולם,\nמצורף הסיכום מהישיבה.\n\nבברכה";
        assert_eq!(parse_pasted_rows(prose), Err(PasteError::TooFewColumns));
        // A stray tab in prose: not a rectangular copy of log rows.
        let stray = "08:00\ta\tb\tc\nסתם שורה של טקסט\n";
        assert_eq!(parse_pasted_rows(stray), Err(PasteError::NotTabular));
        let html = "<table><tr><td>08:00</td><td>a</td></tr></table>";
        assert_eq!(parse_pasted_rows(html), Err(PasteError::TooFewColumns));
    }

    #[test]
    fn rejects_unexpected_column_shapes() {
        let wide = format!("{}\n", vec!["x"; MAX_COLUMNS + 1].join("\t"));
        assert_eq!(parse_pasted_rows(&wide), Err(PasteError::TooManyColumns));
        let header_then_short = "Description\tTo\tFrom\tTime\n09:00\ta\n";
        assert_eq!(parse_pasted_rows(header_then_short), Err(PasteError::NotTabular));
        // Wide but within the limit (whole sheet rows selected) is fine.
        let many = format!("08:00\ta\tb\tc{}\n", "\t".repeat(MAX_COLUMNS - 4));
        assert_eq!(parse_pasted_rows(&many).unwrap().rows, vec![row("08:00", "a", "b", "c")]);
    }

    #[test]
    fn several_pasted_groups_parse_independently() {
        // Non-contiguous selections are pasted one group at a time.
        let first = parse_pasted_rows("08:00\ta\tb\tראשון\r\n").unwrap();
        let second = parse_pasted_rows("11:30\tc\td\tשני\r\n11:40\te\tf\tשלישי\r\n").unwrap();
        let all: Vec<&str> = first.rows.iter().chain(&second.rows).map(|r| r.description.as_str()).collect();
        assert_eq!(all, vec!["ראשון", "שני", "שלישי"]);
    }

    #[test]
    fn keeps_unicode_hebrew_and_punctuation_unchanged() {
        let text = "07:05\tמוקד \"צפון\" (א׳)\tעמדה 3/ב\tנוסח: ״הכול תקין״ – 100% ✓ café\n";
        let pasted = parse_pasted_rows(text).unwrap();
        assert_eq!(pasted.rows[0], row("07:05", "מוקד \"צפון\" (א׳)", "עמדה 3/ב", "נוסח: ״הכול תקין״ – 100% ✓ café"));
        // Directional marks used in real Hebrew text are kept.
        let marked = parse_pasted_rows("07:05\tא\u{200F}ב\tג\tד").unwrap();
        assert_eq!(marked.rows[0].from, "א\u{200F}ב");
    }

    #[test]
    fn strips_invisible_overrides_and_control_characters() {
        let text = "08:00\t\u{202E}evil\u{202C}\tb\u{1B}[31m\t\u{FEFF}ok\u{7}";
        let pasted = parse_pasted_rows(text).unwrap();
        assert_eq!(pasted.rows[0], row("08:00", "evil", "b[31m", "ok"));
    }

    #[test]
    fn script_like_content_stays_literal_text() {
        let text = "08:00\t<script>alert('x')</script>\t=cmd|'/c calc'!A1\t\"<b>bold</b>\n& more\"\r\n";
        let pasted = parse_pasted_rows(text).unwrap();
        assert_eq!(
            pasted.rows[0],
            row("08:00", "<script>alert('x')</script>", "=cmd|'/c calc'!A1", "<b>bold</b>\n& more")
        );
    }

    #[test]
    fn enforces_limits() {
        assert_eq!(parse_pasted_rows(&"x".repeat(MAX_PASTE_BYTES + 1)), Err(PasteError::TooLarge));
        let many = "08:00\ta\tb\tc\n".repeat(MAX_ROWS + 1);
        assert_eq!(parse_pasted_rows(&many), Err(PasteError::TooManyRows));
        let long = format!("08:00\ta\tb\t{}", "x".repeat(MAX_CELL_CHARS + 1));
        assert_eq!(parse_pasted_rows(&long), Err(PasteError::CellTooLong));
        // At the limits it still works.
        let longest = format!("08:00\ta\tb\t{}", "א".repeat(MAX_CELL_CHARS));
        assert_eq!(parse_pasted_rows(&longest).unwrap().rows[0].description.chars().count(), MAX_CELL_CHARS);
        let most = "08:00\ta\tb\tc\n".repeat(MAX_ROWS);
        assert_eq!(parse_pasted_rows(&most).unwrap().rows.len(), MAX_ROWS);
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
