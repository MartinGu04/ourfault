//! PDF rendering of a [`DocumentView`]: A4, right-to-left, with the bundled
//! Hebrew/Latin font embedded.
//!
//! Deliberately small: text lines, field grids and tables with repeated
//! header rows and page breaks inside long rows. Drafts carry a banner on
//! every page and a diagonal watermark, so an incomplete investigation can
//! never pass for a final one.

mod font;
mod text;
mod writer;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use self::font::Font;
use self::text::{layout, Line};
use self::writer::{text_string, PdfWriter};
use super::document::{Block, DocumentView, FieldLine};
use super::labels;

const PAGE_WIDTH: f32 = 595.28;
const PAGE_HEIGHT: f32 = 841.89;
const MARGIN_X: f32 = 42.0;
const TOP: f32 = PAGE_HEIGHT - 44.0;
const BOTTOM: f32 = 54.0;
const RIGHT: f32 = PAGE_WIDTH - MARGIN_X;
const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN_X;
const CELL_PADDING: f32 = 4.0;
const TABLE_TEXT: f32 = 9.0;
const COLUMN_GAP: f32 = 18.0;

const REGULAR: usize = 0;
const BOLD: usize = 1;

#[derive(Clone, Copy)]
struct Color(f32, f32, f32);

const INK: Color = Color(0.1, 0.11, 0.13);
const MUTED: Color = Color(0.4, 0.43, 0.47);
const RULE: Color = Color(0.62, 0.65, 0.69);
const HEADER_FILL: Color = Color(0.92, 0.93, 0.95);
const DRAFT: Color = Color(0.72, 0.12, 0.1);
const DRAFT_FILL: Color = Color(0.99, 0.93, 0.92);
const WATERMARK: Color = Color(0.9, 0.9, 0.91);

pub struct PdfRenderer;

impl PdfRenderer {
    pub fn render(view: &DocumentView) -> Vec<u8> {
        Self::render_traced(view).0
    }

    /// Renders and also returns every text line drawn, in logical order.
    pub(crate) fn render_traced(view: &DocumentView) -> (Vec<u8>, Vec<String>) {
        let mut painter = Painter::new(view.draft_notice.clone());
        painter.document(view);
        painter.finish(&view.title)
    }
}

struct Painter {
    fonts: [&'static Font; 2],
    pages: Vec<String>,
    /// Index of the page being drawn on.
    current: usize,
    y: f32,
    draft_notice: Option<String>,
    used: [BTreeMap<u16, char>; 2],
    trace: Vec<String>,
}

fn line_height(size: f32) -> f32 {
    size * 1.45
}

impl Painter {
    fn new(draft_notice: Option<String>) -> Self {
        let mut painter = Self {
            fonts: [Font::regular(), Font::bold()],
            pages: Vec::new(),
            current: 0,
            y: TOP,
            draft_notice,
            used: [BTreeMap::new(), BTreeMap::new()],
            trace: Vec::new(),
        };
        painter.new_page();
        painter
    }

    fn ops(&mut self) -> &mut String {
        &mut self.pages[self.current]
    }

    fn wrap(&self, font: usize, size: f32, text: &str, width: f32, rtl: bool) -> Vec<Line> {
        layout(self.fonts[font], size, text, width, rtl)
    }

    // ---- drawing primitives ----

    /// Draws `line` with its baseline at `baseline`, starting at `x`.
    fn draw(&mut self, line: &Line, font: usize, size: f32, x: f32, baseline: f32, color: Color) {
        if line.glyphs.is_empty() {
            return;
        }
        let mut hex = String::with_capacity(line.glyphs.len() * 4);
        for (glyph, c) in &line.glyphs {
            let _ = write!(hex, "{glyph:04X}");
            self.used[font].entry(*glyph).or_insert(*c);
        }
        self.trace.push(line.logical.clone());
        let Color(r, g, b) = color;
        let _ = writeln!(
            self.ops(),
            "BT /F{} {size:.2} Tf {r:.3} {g:.3} {b:.3} rg {x:.2} {baseline:.2} Td <{hex}> Tj ET",
            font + 1
        );
    }

    /// Draws `line` so that it ends at `right` (right-aligned).
    fn draw_right(&mut self, line: &Line, font: usize, size: f32, right: f32, baseline: f32, color: Color) {
        self.draw(line, font, size, right - line.width, baseline, color);
    }

    fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, fill: Option<Color>, stroke: Option<Color>) {
        let op = format!("{x:.2} {y:.2} {width:.2} {height:.2} re");
        let op = match (fill, stroke) {
            (Some(Color(r, g, b)), Some(Color(sr, sg, sb))) => {
                format!("{r:.3} {g:.3} {b:.3} rg {sr:.3} {sg:.3} {sb:.3} RG 0.5 w {op} B")
            }
            (Some(Color(r, g, b)), None) => format!("{r:.3} {g:.3} {b:.3} rg {op} f"),
            (None, Some(Color(r, g, b))) => format!("{r:.3} {g:.3} {b:.3} RG 0.5 w {op} S"),
            (None, None) => return,
        };
        let _ = writeln!(self.ops(), "{op}");
    }

    fn rule(&mut self, y: f32, width: f32, color: Color) {
        let Color(r, g, b) = color;
        let _ =
            writeln!(self.ops(), "{r:.3} {g:.3} {b:.3} RG {width:.2} w {MARGIN_X:.2} {y:.2} m {RIGHT:.2} {y:.2} l S");
    }

    // ---- pages ----

    fn new_page(&mut self) {
        self.pages.push(String::new());
        self.current = self.pages.len() - 1;
        self.y = TOP;
        if let Some(notice) = self.draft_notice.clone() {
            self.watermark(&notice);
            let height = 24.0;
            self.rect(MARGIN_X, self.y - height, CONTENT_WIDTH, height, Some(DRAFT_FILL), Some(DRAFT));
            let line = self.wrap(BOLD, 11.0, &notice, CONTENT_WIDTH, true).remove(0);
            let x = (PAGE_WIDTH - line.width) / 2.0;
            self.draw(&line, BOLD, 11.0, x, self.y - 16.0, DRAFT);
            self.y -= height + 14.0;
        }
    }

    /// Large diagonal text across the page, drawn first so content stays on top.
    fn watermark(&mut self, notice: &str) {
        let size = 58.0;
        let line = self.wrap(BOLD, size, notice, 10_000.0, true).remove(0);
        let (sin, cos) = std::f32::consts::FRAC_PI_4.sin_cos();
        let (cx, cy) = (PAGE_WIDTH / 2.0, PAGE_HEIGHT / 2.0);
        let x = cx - line.width / 2.0 * cos + size * 0.3 * sin;
        let y = cy - line.width / 2.0 * sin - size * 0.3 * cos;
        let mut hex = String::new();
        for (glyph, c) in &line.glyphs {
            let _ = write!(hex, "{glyph:04X}");
            self.used[BOLD].entry(*glyph).or_insert(*c);
        }
        let Color(r, g, b) = WATERMARK;
        let _ = writeln!(
            self.ops(),
            "BT /F{} {size:.2} Tf {r:.3} {g:.3} {b:.3} rg {cos:.4} {sin:.4} {:.4} {cos:.4} {x:.2} {y:.2} Tm <{hex}> Tj ET",
            BOLD + 1,
            -sin
        );
    }

    /// Starts a new page unless `height` still fits on the current one.
    fn ensure(&mut self, height: f32) {
        if self.y - height < BOTTOM {
            self.new_page();
        }
    }

    // ---- blocks ----

    fn document(&mut self, view: &DocumentView) {
        for line in self.wrap(BOLD, 20.0, &view.title, CONTENT_WIDTH, true) {
            self.draw_right(&line, BOLD, 20.0, RIGHT, self.y - 18.0, INK);
            self.y -= line_height(20.0);
        }
        for line in self.wrap(REGULAR, 12.5, &view.subtitle, CONTENT_WIDTH, true) {
            self.draw_right(&line, REGULAR, 12.5, RIGHT, self.y - 11.0, MUTED);
            self.y -= line_height(12.5);
        }
        self.y -= 4.0;
        self.rule(self.y, 1.2, INK);
        self.y -= 16.0;

        for block in &view.blocks {
            match block {
                Block::Fields { title, fields } => self.fields(title, fields),
                Block::Table { title, columns, rows, empty_text } => {
                    let headers: Vec<&str> = columns.iter().map(|c| c.label.as_str()).collect();
                    let ltr: Vec<bool> = columns.iter().map(|c| c.ltr).collect();
                    self.table(title, &headers, &ltr, rows, empty_text);
                }
            }
            self.y -= 12.0;
        }
    }

    fn heading(&mut self, title: &str, keep_with: f32) {
        self.ensure(26.0 + keep_with);
        for line in self.wrap(BOLD, 12.5, title, CONTENT_WIDTH, true) {
            self.draw_right(&line, BOLD, 12.5, RIGHT, self.y - 11.0, INK);
            self.y -= line_height(12.5);
        }
        self.rule(self.y + 1.0, 0.5, RULE);
        self.y -= 7.0;
    }

    /// A two-column grid of labelled values (right column first); wide
    /// fields take a whole row.
    fn fields(&mut self, title: &str, fields: &[FieldLine]) {
        let column_width = (CONTENT_WIDTH - COLUMN_GAP) / 2.0;
        let mut rows: Vec<Vec<&FieldLine>> = Vec::new();
        for field in fields {
            match rows.last_mut() {
                Some(row) if row.len() == 1 && !row[0].wide && !field.wide => row.push(field),
                _ => rows.push(vec![field]),
            }
        }
        let mut first = true;
        for row in rows {
            let width = if row.len() == 1 && row[0].wide { CONTENT_WIDTH } else { column_width };
            let laid_out: Vec<(Vec<Line>, Vec<Line>)> = row
                .iter()
                .map(|field| {
                    (
                        self.wrap(REGULAR, 8.5, &field.label, width, true),
                        self.wrap(REGULAR, 10.5, &field.value, width, !field.ltr),
                    )
                })
                .collect();
            let height = laid_out
                .iter()
                .map(|(label, value)| label.len() as f32 * line_height(8.5) + value.len() as f32 * line_height(10.5))
                .fold(0.0, f32::max)
                + 7.0;
            if first {
                self.heading(title, height);
                first = false;
            } else {
                self.ensure(height);
            }
            for (index, (label, value)) in laid_out.iter().enumerate() {
                let right = RIGHT - index as f32 * (column_width + COLUMN_GAP);
                let mut y = self.y;
                for line in label {
                    self.draw_right(line, REGULAR, 8.5, right, y - 7.5, MUTED);
                    y -= line_height(8.5);
                }
                for line in value {
                    self.draw_right(line, REGULAR, 10.5, right, y - 9.5, INK);
                    y -= line_height(10.5);
                }
            }
            self.y -= height;
        }
    }

    /// Column widths: proportional to content, scaled to the page width.
    fn column_widths(&self, headers: &[&str], rows: &[Vec<String>]) -> Vec<f32> {
        let natural: Vec<f32> = headers
            .iter()
            .enumerate()
            .map(|(index, header)| {
                let header = self.fonts[BOLD].width(header, TABLE_TEXT);
                let content = rows
                    .iter()
                    .filter_map(|row| row.get(index))
                    .flat_map(|cell| cell.lines())
                    .map(|line| self.fonts[REGULAR].width(line, TABLE_TEXT))
                    .fold(0.0, f32::max);
                header.max(content).min(220.0) + 2.0 * CELL_PADDING + 4.0
            })
            .map(|width| width.max(40.0))
            .collect();
        let total: f32 = natural.iter().sum();
        natural.iter().map(|width| width * CONTENT_WIDTH / total).collect()
    }

    fn table(&mut self, title: &str, headers: &[&str], ltr: &[bool], rows: &[Vec<String>], empty_text: &str) {
        if rows.is_empty() {
            self.heading(title, line_height(10.0));
            for line in self.wrap(REGULAR, 10.0, empty_text, CONTENT_WIDTH, true) {
                self.draw_right(&line, REGULAR, 10.0, RIGHT, self.y - 9.0, MUTED);
                self.y -= line_height(10.0);
            }
            return;
        }
        let widths = self.column_widths(headers, rows);
        let header_lines: Vec<Vec<Line>> = headers
            .iter()
            .zip(&widths)
            .map(|(header, width)| self.wrap(BOLD, TABLE_TEXT, header, width - 2.0 * CELL_PADDING, true))
            .collect();
        let header_height =
            header_lines.iter().map(Vec::len).max().unwrap_or(1) as f32 * line_height(TABLE_TEXT) + 2.0 * CELL_PADDING;
        self.heading(title, header_height + line_height(TABLE_TEXT) * 2.0 + 2.0 * CELL_PADDING);
        self.table_header(&header_lines, &widths, header_height);

        let lh = line_height(TABLE_TEXT);
        for row in rows {
            let cells: Vec<Vec<Line>> = widths
                .iter()
                .enumerate()
                .map(|(index, width)| {
                    let text = row.get(index).map(String::as_str).unwrap_or_default();
                    self.wrap(
                        REGULAR,
                        TABLE_TEXT,
                        text,
                        width - 2.0 * CELL_PADDING,
                        !ltr.get(index).copied().unwrap_or(false),
                    )
                })
                .collect();
            let total = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let mut offset = 0;
            loop {
                let available = ((self.y - BOTTOM - 2.0 * CELL_PADDING) / lh).floor().max(0.0) as usize;
                // Keep short rows together; split only rows that cannot fit.
                if available == 0 || (offset == 0 && available < total.min(3)) {
                    self.new_page();
                    self.table_header(&header_lines, &widths, header_height);
                    continue;
                }
                let count = available.min(total - offset);
                let height = count as f32 * lh + 2.0 * CELL_PADDING;
                let mut right = RIGHT;
                for (lines, width) in cells.iter().zip(&widths) {
                    self.rect(right - width, self.y - height, *width, height, None, Some(RULE));
                    let mut baseline = self.y - CELL_PADDING - TABLE_TEXT * 0.95;
                    for line in lines.iter().skip(offset).take(count) {
                        self.draw_right(line, REGULAR, TABLE_TEXT, right - CELL_PADDING, baseline, INK);
                        baseline -= lh;
                    }
                    right -= width;
                }
                self.y -= height;
                offset += count;
                if offset >= total {
                    break;
                }
                self.new_page();
                self.table_header(&header_lines, &widths, header_height);
            }
        }
    }

    fn table_header(&mut self, header_lines: &[Vec<Line>], widths: &[f32], height: f32) {
        let mut right = RIGHT;
        for (lines, width) in header_lines.iter().zip(widths) {
            self.rect(right - width, self.y - height, *width, height, Some(HEADER_FILL), Some(RULE));
            let mut baseline = self.y - CELL_PADDING - TABLE_TEXT * 0.95;
            for line in lines {
                self.draw_right(line, BOLD, TABLE_TEXT, right - CELL_PADDING, baseline, INK);
                baseline -= line_height(TABLE_TEXT);
            }
            right -= width;
        }
        self.y -= height;
    }

    // ---- output ----

    fn finish(mut self, title: &str) -> (Vec<u8>, Vec<String>) {
        let count = self.pages.len();
        for index in 0..count {
            let (page, of) = labels::PAGE_OF;
            let footer = format!("{page} {} {of} {count}", index + 1);
            let line = self.wrap(REGULAR, 8.0, &footer, CONTENT_WIDTH, true).remove(0);
            self.current = index;
            self.draw(&line, REGULAR, 8.0, (PAGE_WIDTH - line.width) / 2.0, 30.0, MUTED);
        }

        let mut pdf = PdfWriter::new();
        let catalog = pdf.reserve();
        let pages_id = pdf.reserve();
        let mut font_refs = String::new();
        for (index, font) in self.fonts.iter().enumerate() {
            if !self.used[index].is_empty() {
                let id = embed_font(&mut pdf, font, &self.used[index]);
                let _ = write!(font_refs, "/F{} {id} 0 R ", index + 1);
            }
        }
        let mut kids = Vec::new();
        for content in &self.pages {
            let stream = pdf.add_stream("", content.as_bytes());
            let page = pdf.add(format!(
                "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] \
                 /Resources << /Font << {font_refs}>> >> /Contents {stream} 0 R >>"
            ));
            kids.push(format!("{page} 0 R"));
        }
        pdf.set(pages_id, format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), kids.len()).into_bytes());
        pdf.set(catalog, format!("<< /Type /Catalog /Pages {pages_id} 0 R /Lang (he-IL) >>").into_bytes());
        let info = pdf.add(format!("<< /Title {} /Producer (OurFault) >>", text_string(title)));
        (pdf.finish(catalog, info), self.trace)
    }
}

/// Embeds `font` as a CID-keyed TrueType font (Identity-H encoding, glyph
/// ids as character codes) with widths and a ToUnicode map for the glyphs
/// that were used, so text in the PDF can be searched and copied.
fn embed_font(pdf: &mut PdfWriter, font: &Font, used: &BTreeMap<u16, char>) -> usize {
    let file = pdf.add_stream(&format!("/Length1 {}", font.data.len()), font.data);
    let [x_min, y_min, x_max, y_max] = font.bounding_box();
    let descriptor = pdf.add(format!(
        "<< /Type /FontDescriptor /FontName /{} /Flags 32 /FontBBox [{x_min} {y_min} {x_max} {y_max}] \
         /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {file} 0 R >>",
        font.name,
        font.ascender(),
        font.descender(),
        font.cap_height()
    ));
    let mut widths = String::new();
    for glyph in used.keys() {
        let _ = write!(widths, "{glyph} [{}] ", font.advance(*glyph).round());
    }
    let cid_font = pdf.add(format!(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} \
         /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
         /FontDescriptor {descriptor} 0 R /W [{widths}] /CIDToGIDMap /Identity >>",
        font.name
    ));
    let to_unicode = pdf.add_stream("", to_unicode_cmap(used).as_bytes());
    pdf.add(format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H \
         /DescendantFonts [{cid_font} 0 R] /ToUnicode {to_unicode} 0 R >>",
        font.name
    ))
}

fn to_unicode_cmap(used: &BTreeMap<u16, char>) -> String {
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &char)> = used.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for (glyph, c) in chunk {
            let mut units = [0u16; 2];
            let hex: String = c.encode_utf16(&mut units).iter().map(|unit| format!("{unit:04X}")).collect();
            let _ = writeln!(cmap, "<{glyph:04X}> <{hex}>");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMapResource defineresource pop\nend\nend\n");
    cmap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::configuration::tests::configuration;
    use crate::domain::draft::DraftContent;
    use crate::render::document::tests::{published, view_of};

    fn page_count(pdf: &[u8]) -> usize {
        String::from_utf8_lossy(pdf).matches("/Type /Page ").count()
    }

    /// All stream contents, decompressed.
    fn streams(pdf: &[u8]) -> Vec<Vec<u8>> {
        let mut streams = Vec::new();
        let mut rest = pdf;
        while let Some(start) = find(rest, b"stream\n") {
            let body = &rest[start + 7..];
            let end = find(body, b"\nendstream").unwrap();
            streams.push(miniz_oxide::inflate::decompress_to_vec_zlib(&body[..end]).unwrap());
            rest = &body[end + b"\nendstream".len()..];
        }
        streams
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|window| window == needle)
    }

    #[test]
    fn renders_a_valid_pdf_with_every_part_of_the_investigation() {
        let view = view_of(&published());
        let (pdf, trace) = PdfRenderer::render_traced(&view);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.ends_with("%%EOF\n"));
        assert!(text.contains("/BaseFont /Alef-Regular"));
        assert!(text.contains("/ToUnicode"));
        for expected in
            ["תחקיר 056-2026", "בלט רומני", "מס׳ זנב", "4X-102", "תחנת אורן", "השתלשלות אירועים", "עמוד 1 מתוך"]
        {
            assert!(trace.iter().any(|line| line.contains(expected)), "missing {expected}");
        }
        assert!(!trace.iter().any(|line| line.contains(labels::DRAFT_NOTICE)), "no draft marking");
        let contents: String = streams(&pdf).iter().map(|s| String::from_utf8_lossy(s).into_owned()).collect();
        assert!(!contents.contains(" Tm <"), "no watermark");
        assert!(contents.contains("beginbfchar"), "text can be copied from the PDF");
    }

    #[test]
    fn drafts_are_watermarked_on_every_page() {
        let draft = DraftContent {
            rows: (0..120)
                .map(|i| crate::domain::log_rows::LogRow {
                    time: format!("08:{:02}", i % 60),
                    from: "מוקד".into(),
                    to: "עמדה".into(),
                    description: "שורה ארוכה ".repeat(8),
                })
                .collect(),
            ..DraftContent::default()
        };
        let view = DocumentView::from_draft(&draft, &configuration());
        let (pdf, trace) = PdfRenderer::render_traced(&view);
        let pages = page_count(&pdf);
        assert!(pages > 1, "long chronology spans pages");
        let banners = trace.iter().filter(|line| line.as_str() == labels::DRAFT_NOTICE).count();
        assert_eq!(banners, pages, "a banner on every page (the watermark is drawn separately)");
        let rotated = streams(&pdf).iter().map(|s| String::from_utf8_lossy(s).matches(" Tm <").count()).sum::<usize>();
        assert_eq!(rotated, pages, "a diagonal watermark per page");
        let header_rows = trace.iter().filter(|line| line.as_str() == labels::DESCRIPTION).count();
        assert_eq!(header_rows, pages, "table header repeats on each page");
    }

    #[test]
    fn overlong_cells_continue_on_the_next_page() {
        let draft = DraftContent {
            rows: vec![crate::domain::log_rows::LogRow {
                time: "08:00".into(),
                from: "א".into(),
                to: "ב".into(),
                description: "מילה ".repeat(1800),
            }],
            ..DraftContent::default()
        };
        let (pdf, trace) = PdfRenderer::render_traced(&DocumentView::from_draft(&draft, &configuration()));
        assert!(page_count(&pdf) > 1);
        let words: usize =
            trace.iter().filter(|line| line.starts_with("מילה")).map(|l| l.matches("מילה").count()).sum();
        assert_eq!(words, 1800, "no text is lost at page breaks");
    }

    #[test]
    fn the_font_covers_all_document_wording() {
        let views = [view_of(&published()), DocumentView::from_draft(&DraftContent::default(), &configuration())];
        let mut missing: Vec<char> = Vec::new();
        for text in views.iter().flat_map(DocumentView::all_text) {
            for c in text.chars().filter(|c| !c.is_whitespace()) {
                for font in [Font::regular(), Font::bold()] {
                    if !font.has_glyph(c) && !missing.contains(&c) {
                        missing.push(c);
                    }
                }
            }
        }
        assert!(missing.is_empty(), "font lacks {missing:?}");
    }
}

#[cfg(test)]
/// Writes sample PDFs for a visual check:
/// `SAMPLE_DIR=/some/dir cargo test write_samples -- --ignored`
mod sample_output {
    #[test]
    #[ignore]
    fn write_samples() {
        use super::*;
        let dir = std::path::PathBuf::from(std::env::var("SAMPLE_DIR").unwrap());
        let seeded = crate::seed::investigations();
        let published = seeded.iter().find(|p| p.investigation.number.to_string() == "054-2026").unwrap();
        let view = DocumentView::from_investigation(&published.investigation, &published.lifecycle);
        std::fs::write(dir.join("completed.pdf"), PdfRenderer::render(&view)).unwrap();
        let draft = &crate::seed::drafts()[0];
        let view = DocumentView::from_draft(&draft.content, &crate::seed::configuration());
        std::fs::write(dir.join("draft.pdf"), PdfRenderer::render(&view)).unwrap();
    }
}
