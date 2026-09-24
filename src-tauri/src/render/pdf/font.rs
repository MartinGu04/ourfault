//! The bundled font (Alef, SIL Open Font License, Hebrew and Latin). Only
//! metrics and glyph lookup are needed here; the font file is embedded
//! into every PDF as is.

use std::sync::OnceLock;

use ttf_parser::{Face, GlyphId};

static REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Alef-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../../../assets/fonts/Alef-Bold.ttf");

pub struct Font {
    /// PostScript-style name used in the PDF.
    pub name: &'static str,
    pub data: &'static [u8],
    face: Face<'static>,
    /// Font units → thousandths of an em (PDF glyph space).
    scale: f32,
}

impl Font {
    fn load(name: &'static str, data: &'static [u8]) -> Self {
        let face = Face::parse(data, 0).expect("the bundled font is a valid TrueType font");
        let scale = 1000.0 / f32::from(face.units_per_em());
        Self { name, data, face, scale }
    }

    pub fn regular() -> &'static Font {
        static FONT: OnceLock<Font> = OnceLock::new();
        FONT.get_or_init(|| Font::load("Alef-Regular", REGULAR))
    }

    pub fn bold() -> &'static Font {
        static FONT: OnceLock<Font> = OnceLock::new();
        FONT.get_or_init(|| Font::load("Alef-Bold", BOLD))
    }

    pub fn has_glyph(&self, c: char) -> bool {
        self.face.glyph_index(c).is_some()
    }

    /// Glyph id for `c` (0, the "missing glyph", if the font lacks it).
    pub fn glyph(&self, c: char) -> u16 {
        self.face.glyph_index(c).map_or(0, |glyph| glyph.0)
    }

    /// Advance width in thousandths of an em.
    pub fn advance(&self, glyph: u16) -> f32 {
        f32::from(self.face.glyph_hor_advance(GlyphId(glyph)).unwrap_or(0)) * self.scale
    }

    /// Width of `text` in points at `size`.
    pub fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.advance(self.glyph(c))).sum::<f32>() * size / 1000.0
    }

    pub fn scaled(&self, value: i16) -> i32 {
        (f32::from(value) * self.scale).round() as i32
    }

    pub fn ascender(&self) -> i32 {
        self.scaled(self.face.ascender())
    }

    pub fn descender(&self) -> i32 {
        self.scaled(self.face.descender())
    }

    pub fn cap_height(&self) -> i32 {
        self.face.capital_height().map_or(700, |height| self.scaled(height))
    }

    pub fn bounding_box(&self) -> [i32; 4] {
        let bbox = self.face.global_bounding_box();
        [self.scaled(bbox.x_min), self.scaled(bbox.y_min), self.scaled(bbox.x_max), self.scaled(bbox.y_max)]
    }
}

/// Replaces characters the font cannot draw with close equivalents, and
/// anything else with '?', so no text silently disappears.
pub fn drawable(font: &Font, c: char) -> char {
    if font.has_glyph(c) {
        return c;
    }
    let substitute = match c {
        '\u{05F3}' => '\'',
        '\u{05F4}' => '"',
        '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
        '\u{00A0}' | '\u{2007}' | '\u{202F}' => ' ',
        _ => '?',
    };
    if font.has_glyph(substitute) {
        substitute
    } else {
        '?'
    }
}
