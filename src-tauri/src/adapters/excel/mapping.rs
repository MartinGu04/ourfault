//! Maps a raw worksheet grid to operations-log rows: locates the header row,
//! resolves columns by name, converts values to display text and sanitises
//! them. Everything here treats workbook content as untrusted text.

use super::grid::{Cell, DateTimeParts, RawRow};

/// Maximum characters kept per cell; longer values are truncated.
pub const MAX_CELL_CHARS: usize = 2000;
/// Maximum data rows accepted from one workbook.
pub const MAX_DATA_ROWS: usize = 5000;
/// How many rows from the top are searched for the header row, allowing for
/// title rows above it.
const HEADER_SEARCH_ROWS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Time,
    From,
    To,
    Description,
    EventType,
}

impl Column {
    pub const REQUIRED: [Column; 4] = [Column::Time, Column::From, Column::To, Column::Description];

    /// Stable key reported to the UI for missing columns.
    pub fn key(self) -> &'static str {
        match self {
            Column::Time => "time",
            Column::From => "from",
            Column::To => "to",
            Column::Description => "description",
            Column::EventType => "eventType",
        }
    }

    /// Accepted header titles (normalised: lower-case, single spaces).
    fn aliases(self) -> &'static [&'static str] {
        match self {
            Column::Time => &["time", "שעה", "זמן"],
            Column::From => &["from", "ממי", "מאת"],
            Column::To => &["to", "למי", "אל"],
            Column::Description => &["description", "תוכן", "תיאור"],
            Column::EventType => &["event type", "סוג אירוע"],
        }
    }

    fn all() -> [Column; 5] {
        [Column::Time, Column::From, Column::To, Column::Description, Column::EventType]
    }
}

/// A mapped row before highlight information is attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedRow {
    pub number: u32,
    pub time: String,
    pub from: String,
    pub to: String,
    pub description: String,
    pub event_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MappingError {
    #[error("header row not found, missing columns: {0:?}")]
    MissingColumns(Vec<&'static str>),
    #[error("worksheet has no data rows")]
    NoRows,
    #[error("worksheet has more than {MAX_DATA_ROWS} data rows")]
    TooManyRows,
}

#[derive(Debug, Default)]
struct ColumnIndexes {
    time: Option<usize>,
    from: Option<usize>,
    to: Option<usize>,
    description: Option<usize>,
    event_type: Option<usize>,
}

impl ColumnIndexes {
    fn get(&self, column: Column) -> Option<usize> {
        match column {
            Column::Time => self.time,
            Column::From => self.from,
            Column::To => self.to,
            Column::Description => self.description,
            Column::EventType => self.event_type,
        }
    }

    fn slot(&mut self, column: Column) -> &mut Option<usize> {
        match column {
            Column::Time => &mut self.time,
            Column::From => &mut self.from,
            Column::To => &mut self.to,
            Column::Description => &mut self.description,
            Column::EventType => &mut self.event_type,
        }
    }

    fn missing_required(&self) -> Vec<&'static str> {
        Column::REQUIRED.iter().filter(|c| self.get(**c).is_none()).map(|c| c.key()).collect()
    }
}

pub fn map_rows(rows: &[RawRow]) -> Result<Vec<MappedRow>, MappingError> {
    let (header_position, columns) = find_header(rows)?;

    let mut mapped = Vec::new();
    for row in &rows[header_position + 1..] {
        let value = |column: Column| -> String {
            columns.get(column).and_then(|i| row.cells.get(i)).map(|cell| cell_text(cell, column)).unwrap_or_default()
        };
        let candidate = MappedRow {
            number: row.number,
            time: value(Column::Time),
            from: value(Column::From),
            to: value(Column::To),
            description: value(Column::Description),
            event_type: value(Column::EventType),
        };
        let is_blank = [&candidate.time, &candidate.from, &candidate.to, &candidate.description]
            .iter()
            .all(|text| text.is_empty());
        if is_blank {
            continue;
        }
        if mapped.len() == MAX_DATA_ROWS {
            return Err(MappingError::TooManyRows);
        }
        mapped.push(candidate);
    }

    if mapped.is_empty() {
        return Err(MappingError::NoRows);
    }
    Ok(mapped)
}

/// Returns the position of the header row and its column indexes. When no
/// row qualifies, reports the columns missing from the best candidate.
fn find_header(rows: &[RawRow]) -> Result<(usize, ColumnIndexes), MappingError> {
    let mut best_missing: Option<Vec<&'static str>> = None;
    for (position, row) in rows.iter().take(HEADER_SEARCH_ROWS).enumerate() {
        let columns = header_columns(row);
        let missing = columns.missing_required();
        if missing.is_empty() {
            return Ok((position, columns));
        }
        if best_missing.as_ref().is_none_or(|best| missing.len() < best.len()) {
            best_missing = Some(missing);
        }
    }
    let all_required = Column::REQUIRED.iter().map(|c| c.key()).collect();
    Err(MappingError::MissingColumns(best_missing.unwrap_or(all_required)))
}

fn header_columns(row: &RawRow) -> ColumnIndexes {
    let mut columns = ColumnIndexes::default();
    for (index, cell) in row.cells.iter().enumerate() {
        let Cell::Text(text) = cell else { continue };
        let title = normalise_header(text);
        for column in Column::all() {
            let slot = columns.slot(column);
            if slot.is_none() && column.aliases().contains(&title.as_str()) {
                *slot = Some(index);
            }
        }
    }
    columns
}

fn normalise_header(text: &str) -> String {
    text.trim().trim_end_matches(':').split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn cell_text(cell: &Cell, column: Column) -> String {
    match cell {
        Cell::Empty => String::new(),
        Cell::Text(text) => sanitise(text),
        Cell::DateTime(parts) => format_date_time(parts),
        // A bare fraction in the time column is an unformatted Excel time.
        Cell::Number(value) if column == Column::Time && (0.0..1.0).contains(value) => {
            format_date_time(&time_from_fraction(*value))
        }
        Cell::Number(value) => format_number(*value),
    }
}

fn time_from_fraction(fraction: f64) -> DateTimeParts {
    let seconds = (fraction * 86_400.0).round() as u32 % 86_400;
    DateTimeParts {
        date: None,
        hour: (seconds / 3600) as u8,
        minute: (seconds / 60 % 60) as u8,
        second: (seconds % 60) as u8,
    }
}

/// `HH:MM`, or `HH:MM:SS` when seconds are present, prefixed with
/// `DD/MM/YYYY` for full date-time values.
fn format_date_time(parts: &DateTimeParts) -> String {
    let time = if parts.second == 0 {
        format!("{:02}:{:02}", parts.hour, parts.minute)
    } else {
        format!("{:02}:{:02}:{:02}", parts.hour, parts.minute, parts.second)
    };
    match parts.date {
        None => time,
        Some((year, month, day)) => format!("{day:02}/{month:02}/{year} {time}"),
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

/// Trims, drops control characters other than line breaks and tabs,
/// normalises line endings and caps the length. The result is always
/// rendered as plain text by the UI.
fn sanitise(text: &str) -> String {
    let cleaned: String = text
        .trim()
        .replace("\r\n", "\n")
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(MAX_CELL_CHARS)
        .collect();
    cleaned.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Cell {
        Cell::Text(value.into())
    }

    fn row(number: u32, cells: Vec<Cell>) -> RawRow {
        RawRow { number, cells }
    }

    fn english_header() -> RawRow {
        row(1, vec![text("Time"), text("From"), text("To"), text("Description"), text("Event type")])
    }

    #[test]
    fn maps_rows_by_header_name() {
        let rows = vec![
            english_header(),
            row(2, vec![text("08:15"), text("מוקד"), text("עמדה 3"), text("דיווח על תקלה"), text("תקלה")]),
        ];
        let mapped = map_rows(&rows).unwrap();
        assert_eq!(
            mapped,
            vec![MappedRow {
                number: 2,
                time: "08:15".into(),
                from: "מוקד".into(),
                to: "עמדה 3".into(),
                description: "דיווח על תקלה".into(),
                event_type: "תקלה".into(),
            }]
        );
    }

    #[test]
    fn accepts_hebrew_headers_in_any_order_below_a_title_row() {
        let rows = vec![
            row(1, vec![text("יומן מבצעים – משמרת בוקר")]),
            row(2, vec![text("תוכן"), text("למי"), text(" ממי "), text("שעה:")]),
            row(3, vec![text("שיחה"), text("ב"), text("א"), text("09:00")]),
        ];
        let mapped = map_rows(&rows).unwrap();
        assert_eq!(mapped[0].number, 3);
        assert_eq!((mapped[0].from.as_str(), mapped[0].to.as_str()), ("א", "ב"));
        assert_eq!(mapped[0].description, "שיחה");
        assert_eq!(mapped[0].event_type, "", "event type is optional");
    }

    #[test]
    fn reports_missing_required_columns() {
        let rows = vec![row(1, vec![text("Time"), text("From"), text("Notes")]), row(2, vec![text("x")])];
        assert_eq!(map_rows(&rows), Err(MappingError::MissingColumns(vec!["to", "description"])));
    }

    #[test]
    fn converts_excel_times_and_dates() {
        let time = |hour, minute, second| DateTimeParts { date: None, hour, minute, second };
        let rows = vec![
            english_header(),
            row(2, vec![Cell::DateTime(time(8, 5, 0)), text("a"), text("b"), text("c")]),
            row(3, vec![Cell::DateTime(time(8, 5, 30)), text("a"), text("b"), text("c")]),
            row(4, vec![Cell::Number(0.5), text("a"), text("b"), Cell::Number(42.0)]),
            row(
                5,
                vec![
                    Cell::DateTime(DateTimeParts { date: Some((2026, 9, 23)), ..time(14, 0, 0) }),
                    text("a"),
                    text("b"),
                    Cell::Number(1.5),
                ],
            ),
        ];
        let mapped = map_rows(&rows).unwrap();
        let times: Vec<&str> = mapped.iter().map(|r| r.time.as_str()).collect();
        assert_eq!(times, vec!["08:05", "08:05:30", "12:00", "23/09/2026 14:00"]);
        assert_eq!(mapped[2].description, "42");
        assert_eq!(mapped[3].description, "1.5");
    }

    #[test]
    fn skips_blank_rows_and_sanitises_text() {
        let rows = vec![
            english_header(),
            row(2, vec![Cell::Empty, text("  "), Cell::Empty, Cell::Empty, text("שגרה")]),
            row(3, vec![text("10:00"), text("a\u{0}b"), text("c"), text("line 1\r\nline 2  ")]),
        ];
        let mapped = map_rows(&rows).unwrap();
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].from, "ab");
        assert_eq!(mapped[0].description, "line 1\nline 2");
    }

    #[test]
    fn keeps_markup_as_literal_text() {
        let rows = vec![
            english_header(),
            row(2, vec![text("10:00"), text("<img src=x onerror=alert(1)>"), text("=1+1"), text("c")]),
        ];
        let mapped = map_rows(&rows).unwrap();
        assert_eq!(mapped[0].from, "<img src=x onerror=alert(1)>");
        assert_eq!(mapped[0].to, "=1+1");
    }

    #[test]
    fn truncates_very_long_cells() {
        let rows = vec![english_header(), row(2, vec![text("10:00"), text("a"), text("b"), text(&"x".repeat(5000))])];
        assert_eq!(map_rows(&rows).unwrap()[0].description.chars().count(), MAX_CELL_CHARS);
    }

    #[test]
    fn enforces_row_limits() {
        assert_eq!(map_rows(&[english_header()]), Err(MappingError::NoRows));
        let mut rows = vec![english_header()];
        rows.extend(
            (0..=MAX_DATA_ROWS as u32).map(|i| row(i + 2, vec![text("10:00"), text("a"), text("b"), text("c")])),
        );
        assert_eq!(map_rows(&rows), Err(MappingError::TooManyRows));
    }
}
