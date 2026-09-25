//! Line breaking and bidirectional reordering.
//!
//! PDF text is drawn in visual order. Each paragraph is analysed once with
//! the Unicode Bidirectional Algorithm (right-to-left base direction for
//! Hebrew text), broken into lines by measured width, and every line is
//! reordered from its paragraph's levels. Hebrew needs no glyph shaping;
//! brackets inside right-to-left runs are mirrored.

use std::ops::Range;

use unicode_bidi::{BidiInfo, Level};

use super::font::{drawable, Font};

/// One laid-out line: glyphs in visual (left-to-right drawing) order.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub glyphs: Vec<(u16, char)>,
    /// Width in points.
    pub width: f32,
    /// The line's text in logical order (for diagnostics and tests).
    pub logical: String,
}

impl Line {
    #[cfg(test)]
    pub fn visual(&self) -> String {
        self.glyphs.iter().map(|(_, c)| *c).collect()
    }
}

/// Lays out `text` in lines no wider than `max_width` points. Line breaks
/// in the text are kept; every paragraph produces at least one line.
pub fn layout(font: &Font, size: f32, text: &str, max_width: f32, rtl: bool) -> Vec<Line> {
    let mut lines = Vec::new();
    for paragraph in text.replace("\r\n", "\n").split(['\n', '\r']) {
        let cleaned: String = paragraph
            .chars()
            .filter_map(|c| match c {
                '\t' => Some(' '),
                c if c.is_control() => None,
                c => Some(drawable(font, c)),
            })
            .collect();
        if cleaned.trim().is_empty() {
            lines.push(Line { glyphs: Vec::new(), width: 0.0, logical: String::new() });
            continue;
        }
        let level = if rtl { Level::rtl() } else { Level::ltr() };
        let info = BidiInfo::new(&cleaned, Some(level));
        let para = &info.paragraphs[0];
        for range in break_lines(&cleaned, max_width, |s| font.width(s, size)) {
            let visual = reorder(&info, para, range.clone());
            let glyphs: Vec<(u16, char)> = visual.chars().map(|c| (font.glyph(c), c)).collect();
            let width = glyphs.iter().map(|(glyph, _)| font.advance(*glyph)).sum::<f32>() * size / 1000.0;
            lines.push(Line { glyphs, width, logical: cleaned[range].to_owned() });
        }
    }
    lines
}

/// Greedy line breaking at spaces; words longer than a line are split.
/// Returned ranges exclude the spaces at the break.
fn break_lines(text: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut width = 0.0;
    let mut position = 0;
    for word in text.split_inclusive(' ') {
        let word_start = position;
        position += word.len();
        let visible = word.trim_end_matches(' ');
        let visible_width = measure(visible);
        if width > 0.0 && width + visible_width > max_width {
            lines.push(start..trim_end(text, start, word_start));
            start = word_start;
            width = 0.0;
        }
        if visible_width > max_width {
            // A single overlong word: break it between characters.
            for (offset, c) in visible.char_indices() {
                let char_width = measure(c.encode_utf8(&mut [0; 4]));
                if width > 0.0 && width + char_width > max_width {
                    lines.push(start..word_start + offset);
                    start = word_start + offset;
                    width = 0.0;
                }
                width += char_width;
            }
            width += measure(&word[visible.len()..]);
        } else {
            width += measure(word);
        }
    }
    let end = trim_end(text, start, text.len());
    if end > start || lines.is_empty() {
        lines.push(start..end);
    }
    lines
}

fn trim_end(text: &str, start: usize, end: usize) -> usize {
    start + text[start..end].trim_end_matches(' ').len()
}

/// The characters of `line` in visual order.
fn reorder(info: &BidiInfo<'_>, para: &unicode_bidi::ParagraphInfo, line: Range<usize>) -> String {
    let (levels, runs) = info.visual_runs(para, line);
    let mut visual = String::new();
    for run in runs {
        let text = &info.text[run.clone()];
        if levels[run.start].is_rtl() {
            visual.extend(text.chars().rev().map(mirror));
        } else {
            visual.push_str(text);
        }
    }
    visual
}

fn mirror(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visual(text: &str) -> String {
        layout(Font::regular(), 10.0, text, 10_000.0, true).iter().map(Line::visual).collect::<Vec<_>>().join("|")
    }

    #[test]
    fn reverses_hebrew_and_keeps_numbers_and_latin_readable() {
        assert_eq!(visual("שלום"), "םולש");
        assert_eq!(visual("תחקיר 056-2026"), "056-2026 ריקחת");
        assert_eq!(visual("מערכת Alpha פעילה"), "הליעפ Alpha תכרעמ");
    }

    #[test]
    fn mirrors_brackets_in_right_to_left_text() {
        assert_eq!(visual("איוש בכיר (נגד)"), "(דגנ) ריכב שויא");
    }

    #[test]
    fn keeps_explicit_line_breaks() {
        assert_eq!(visual("אחת\nשתיים"), "תחא|םייתש");
        assert_eq!(layout(Font::regular(), 10.0, "", 100.0, true).len(), 1);
    }

    #[test]
    fn wraps_by_width_and_splits_overlong_words() {
        let measure = |s: &str| s.chars().count() as f32;
        let lines = break_lines("aaa bbb ccc", 7.0, measure);
        let text = "aaa bbb ccc";
        let parts: Vec<&str> = lines.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(parts, vec!["aaa bbb", "ccc"]);

        let text = "abcdefghij";
        let parts: Vec<&str> = break_lines(text, 4.0, measure).iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(parts, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wrapped_hebrew_lines_start_at_the_logical_beginning() {
        let font = Font::regular();
        let text = "השתלשלות אירועים ארוכה מאוד";
        let width = font.width("השתלשלות אירועים", 10.0) + 1.0;
        let lines = layout(font, 10.0, text, width, true);
        let logical: Vec<&str> = lines.iter().map(|l| l.logical.as_str()).collect();
        assert_eq!(logical, vec!["השתלשלות אירועים", "ארוכה מאוד"]);
        assert!(lines.iter().all(|l| l.width <= width));
    }

    #[test]
    fn replaces_undrawable_characters() {
        let line = &layout(Font::regular(), 10.0, "a\u{1F600}b", 1000.0, false)[0];
        assert_eq!(line.visual(), "a?b");
    }
}
