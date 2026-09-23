//! Detects rows whose cells carry a given solid background colour.
//!
//! calamine does not expose cell styles, so this reads the three relevant
//! parts of the .xlsx package directly: the first sheet's location, the fill
//! and cell-format tables in `styles.xml`, and the style index of each cell.
//!
//! Scope, deliberately small: only explicit RGB fills are matched. Theme and
//! legacy indexed colours, conditional formatting and table styles are not
//! resolved; such rows are simply not pre-selected and the operator selects
//! them manually. Failures here never fail the import.

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Cursor, Read, Seek};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use zip::ZipArchive;

/// Cap on decompressed bytes read from any single package part.
const MAX_PART_BYTES: u64 = 64 * 1024 * 1024;

/// A colour as six upper-case hex digits (`RRGGBB`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbColor(String);

impl RgbColor {
    /// Parses `RRGGBB`, `#RRGGBB` or Excel's `AARRGGBB`.
    pub fn parse(value: &str) -> Option<Self> {
        let hex = value.trim().trim_start_matches('#');
        let rgb = match hex.len() {
            6 => hex,
            8 => &hex[2..],
            _ => return None,
        };
        rgb.chars().all(|c| c.is_ascii_hexdigit()).then(|| Self(rgb.to_ascii_uppercase()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FillError {
    #[error("package: {0}")]
    Package(String),
    #[error("xml: {0}")]
    Xml(String),
    #[error("missing part: {0}")]
    MissingPart(String),
}

impl From<zip::result::ZipError> for FillError {
    fn from(error: zip::result::ZipError) -> Self {
        FillError::Package(error.to_string())
    }
}

impl From<quick_xml::Error> for FillError {
    fn from(error: quick_xml::Error) -> Self {
        FillError::Xml(error.to_string())
    }
}

/// Returns the 1-based numbers of rows in the first worksheet that contain at
/// least one cell (or a row-level format) filled with `color`.
pub fn highlighted_rows(bytes: &[u8], color: &RgbColor) -> Result<HashSet<u32>, FillError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    let sheet_path = first_sheet_path(&mut archive)?;
    let highlighted_styles = highlighted_style_indexes(&mut archive, color)?;
    if highlighted_styles.is_empty() {
        return Ok(HashSet::new());
    }
    rows_with_styles(&mut archive, &sheet_path, &highlighted_styles)
}

fn open_part<'a, R: Read + Seek>(
    archive: &'a mut ZipArchive<R>,
    name: &str,
) -> Result<Reader<BufReader<std::io::Take<zip::read::ZipFile<'a, R>>>>, FillError> {
    let file = archive.by_name(name).map_err(|_| FillError::MissingPart(name.to_owned()))?;
    let mut reader = Reader::from_reader(BufReader::new(file.take(MAX_PART_BYTES)));
    reader.config_mut().trim_text(true);
    Ok(reader)
}

fn attribute(element: &BytesStart<'_>, local_name: &[u8]) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attr| attr.key.local_name().as_ref() == local_name)
        .map(|attr| String::from_utf8_lossy(&attr.value).into_owned())
}

enum Tag<'a> {
    /// A start or self-closing element.
    Open(&'a BytesStart<'a>),
    /// The local name of an end element.
    Close(&'a [u8]),
}

/// Streams the part and calls `visit` for every element boundary.
fn for_each_tag<B: BufRead>(reader: &mut Reader<B>, mut visit: impl FnMut(Tag<'_>)) -> Result<(), FillError> {
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(element) | Event::Empty(element) => visit(Tag::Open(&element)),
            Event::End(element) => visit(Tag::Close(element.local_name().into_inner())),
            Event::Eof => return Ok(()),
            _ => {}
        }
        buffer.clear();
    }
}

/// Calls `visit` for every start or self-closing element.
fn for_each_element<B: BufRead>(
    reader: &mut Reader<B>,
    mut visit: impl FnMut(&BytesStart<'_>),
) -> Result<(), FillError> {
    for_each_tag(reader, |tag| {
        if let Tag::Open(element) = tag {
            visit(element);
        }
    })
}

fn first_sheet_path<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<String, FillError> {
    let mut relationship_id = None;
    for_each_element(&mut open_part(archive, "xl/workbook.xml")?, |element| {
        if relationship_id.is_none() && element.local_name().as_ref() == b"sheet" {
            relationship_id = attribute(element, b"id");
        }
    })?;
    let relationship_id = relationship_id.ok_or_else(|| FillError::MissingPart("first sheet".into()))?;

    let mut target = None;
    for_each_element(&mut open_part(archive, "xl/_rels/workbook.xml.rels")?, |element| {
        if target.is_none()
            && element.local_name().as_ref() == b"Relationship"
            && attribute(element, b"Id").as_deref() == Some(relationship_id.as_str())
        {
            target = attribute(element, b"Target");
        }
    })?;
    let target = target.ok_or_else(|| FillError::MissingPart("sheet relationship".into()))?;
    Ok(match target.strip_prefix('/') {
        Some(absolute) => absolute.to_owned(),
        None => format!("xl/{target}"),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Fills,
    CellFormats,
    Other,
}

/// Indexes into `cellXfs` whose fill is a solid fill of `color`.
fn highlighted_style_indexes<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    color: &RgbColor,
) -> Result<HashSet<usize>, FillError> {
    let mut fill_matches: Vec<bool> = Vec::new();
    let mut style_fill_ids: Vec<Option<usize>> = Vec::new();
    // Only `<fills>` and `<cellXfs>` matter; `<xf>` also appears in `<cellStyleXfs>`.
    let mut section = Section::Other;
    let mut solid_pattern = false;

    for_each_tag(&mut open_part(archive, "xl/styles.xml")?, |tag| {
        let element = match tag {
            Tag::Close(b"fills" | b"cellXfs") => {
                section = Section::Other;
                return;
            }
            Tag::Close(_) => return,
            Tag::Open(element) => element,
        };
        match (element.local_name().as_ref(), section) {
            (b"fills", _) => section = Section::Fills,
            (b"cellXfs", _) => section = Section::CellFormats,
            (b"fill", Section::Fills) => fill_matches.push(false),
            (b"patternFill", Section::Fills) => {
                solid_pattern = attribute(element, b"patternType").as_deref() == Some("solid");
            }
            (b"fgColor", Section::Fills) if solid_pattern => {
                let matches = attribute(element, b"rgb").and_then(|rgb| RgbColor::parse(&rgb)).as_ref() == Some(color);
                if let Some(last) = fill_matches.last_mut() {
                    *last = matches;
                }
            }
            (b"xf", Section::CellFormats) => {
                style_fill_ids.push(attribute(element, b"fillId").and_then(|id| id.parse().ok()));
            }
            _ => {}
        }
    })?;

    Ok(style_fill_ids
        .iter()
        .enumerate()
        .filter(|(_, fill_id)| fill_id.is_some_and(|id| fill_matches.get(id).copied().unwrap_or(false)))
        .map(|(style_index, _)| style_index)
        .collect())
}

fn rows_with_styles<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    sheet_path: &str,
    styles: &HashSet<usize>,
) -> Result<HashSet<u32>, FillError> {
    let mut rows = HashSet::new();
    let mut current_row: u32 = 0;
    let has_style = |element: &BytesStart<'_>| {
        attribute(element, b"s").and_then(|s| s.parse::<usize>().ok()).is_some_and(|s| styles.contains(&s))
    };

    for_each_element(&mut open_part(archive, sheet_path)?, |element| match element.local_name().as_ref() {
        b"row" => {
            // `r` is optional; rows without it follow the previous row.
            current_row = attribute(element, b"r").and_then(|r| r.parse().ok()).unwrap_or(current_row + 1);
            let row_formatted = attribute(element, b"customFormat").as_deref() == Some("1");
            if row_formatted && has_style(element) {
                rows.insert(current_row);
            }
        }
        b"c" if has_style(element) => {
            rows.insert(current_row);
        }
        _ => {}
    })?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colours_in_common_notations() {
        let yellow = RgbColor::parse("FFFF00").unwrap();
        assert_eq!(RgbColor::parse("#ffff00"), Some(yellow.clone()));
        assert_eq!(RgbColor::parse("FFFFFF00"), Some(yellow));
        assert_eq!(RgbColor::parse("FFF"), None);
        assert_eq!(RgbColor::parse("GGGGGG"), None);
    }

    #[test]
    fn rejects_non_zip_input() {
        let color = RgbColor::parse("FFFF00").unwrap();
        assert!(highlighted_rows(b"not a workbook", &color).is_err());
    }
}
