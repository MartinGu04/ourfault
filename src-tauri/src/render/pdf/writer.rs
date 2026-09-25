//! Low-level PDF serialisation: numbered objects, compressed streams and
//! the cross-reference table.

use std::fmt::Write as _;

pub struct PdfWriter {
    objects: Vec<Option<Vec<u8>>>,
}

impl PdfWriter {
    pub fn new() -> Self {
        Self { objects: Vec::new() }
    }

    /// Reserves an object number to be filled in later with [`Self::set`].
    pub fn reserve(&mut self) -> usize {
        self.objects.push(None);
        self.objects.len()
    }

    pub fn set(&mut self, id: usize, body: Vec<u8>) {
        self.objects[id - 1] = Some(body);
    }

    pub fn add(&mut self, body: impl Into<Vec<u8>>) -> usize {
        self.objects.push(Some(body.into()));
        self.objects.len()
    }

    /// A Flate-compressed stream object. `dict` holds extra dictionary
    /// entries (without the surrounding `<< >>`).
    pub fn add_stream(&mut self, dict: &str, data: &[u8]) -> usize {
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
        let mut body = format!("<< {dict} /Length {} /Filter /FlateDecode >>\nstream\n", compressed.len()).into_bytes();
        body.extend_from_slice(&compressed);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }

    pub fn finish(self, root: usize, info: usize) -> Vec<u8> {
        let mut out: Vec<u8> = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::with_capacity(self.objects.len());
        for (index, object) in self.objects.into_iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            out.extend_from_slice(&object.expect("every reserved PDF object is filled"));
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
        for offset in &offsets {
            let _ = writeln!(table, "{offset:010} 00000 n ");
        }
        let _ = write!(
            table,
            "trailer\n<< /Size {} /Root {root} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        );
        out.extend_from_slice(table.as_bytes());
        out
    }
}

/// A PDF text string in UTF-16BE with byte-order mark, as hex.
pub fn text_string(text: &str) -> String {
    let mut hex = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(hex, "{unit:04X}");
    }
    hex.push('>');
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_reference_offsets_point_at_objects() {
        let mut writer = PdfWriter::new();
        let reserved = writer.reserve();
        let info = writer.add("<< /Producer (test) >>");
        writer.set(reserved, b"<< /Type /Catalog >>".to_vec());
        let pdf = writer.finish(reserved, info);
        // Byte offsets: search the raw bytes (the header has binary bytes).
        let tail = String::from_utf8(pdf[pdf.len() - 200..].to_vec()).unwrap();
        let xref_at: usize = tail.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(pdf[xref_at..].starts_with(b"xref\n0 3\n"));
        let table = String::from_utf8(pdf[xref_at..].to_vec()).unwrap();
        for (index, line) in table.lines().skip(3).take(2).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            assert!(pdf[offset..].starts_with(format!("{} 0 obj", index + 1).as_bytes()));
        }
        assert!(pdf.ends_with(b"%%EOF\n"));
    }

    #[test]
    fn encodes_unicode_text_strings() {
        assert_eq!(text_string("א1"), "<FEFF05D00031>");
    }
}
