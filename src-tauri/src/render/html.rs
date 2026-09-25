//! Semantic, right-to-left HTML for a [`DocumentView`].
//!
//! Internal output format only: it is what a SharePoint adapter writes into a
//! rich-text field when the list has one body field. Operators never see or
//! copy it. Every value is escaped; nothing from the investigation is ever
//! emitted as markup.

use super::document::{Block, DocumentView, FieldLine};
use crate::domain::mail::escape_html;

pub struct HtmlRenderer;

impl HtmlRenderer {
    /// An HTML fragment (`<article>`) suitable for a rich-text field.
    pub fn render_fragment(view: &DocumentView) -> String {
        let mut html = String::from("<article dir=\"rtl\" lang=\"he\">\n");
        html.push_str("<header>\n");
        if let Some(notice) = &view.draft_notice {
            html.push_str(&format!("<p role=\"note\"><strong>{}</strong></p>\n", escape_html(notice)));
        }
        html.push_str(&format!("<h1>{}</h1>\n", escape_html(&view.title)));
        html.push_str(&format!("<p>{}</p>\n", escape_html(&view.subtitle)));
        html.push_str("</header>\n");
        for block in &view.blocks {
            match block {
                Block::Fields { title, fields } => {
                    html.push_str(&format!("<section>\n<h2>{}</h2>\n<dl>\n", escape_html(title)));
                    for field in fields {
                        html.push_str(&format!("<dt>{}</dt><dd>{}</dd>\n", escape_html(&field.label), value(field)));
                    }
                    html.push_str("</dl>\n</section>\n");
                }
                Block::Table { title, columns, rows, empty_text } => {
                    html.push_str(&format!("<section>\n<h2>{}</h2>\n", escape_html(title)));
                    if rows.is_empty() {
                        html.push_str(&format!("<p>{}</p>\n", escape_html(empty_text)));
                    } else {
                        html.push_str("<table>\n<thead><tr>");
                        for column in columns {
                            html.push_str(&format!("<th scope=\"col\">{}</th>", escape_html(&column.label)));
                        }
                        html.push_str("</tr></thead>\n<tbody>\n");
                        for row in rows {
                            html.push_str("<tr>");
                            for (index, cell) in row.iter().enumerate() {
                                let ltr = columns.get(index).is_some_and(|c| c.ltr) && !cell.is_empty();
                                html.push_str(&format!("<td>{}</td>", text(cell, ltr)));
                            }
                            html.push_str("</tr>\n");
                        }
                        html.push_str("</tbody>\n</table>\n");
                    }
                    html.push_str("</section>\n");
                }
            }
        }
        html.push_str("</article>");
        html
    }
}

fn value(field: &FieldLine) -> String {
    text(&field.value, field.ltr)
}

/// Escaped text; multi-line values keep their line breaks; left-to-right
/// values are isolated so they do not disturb the right-to-left layout.
fn text(value: &str, ltr: bool) -> String {
    let escaped = escape_html(value).replace('\n', "<br>");
    if ltr {
        format!("<bdi dir=\"ltr\">{escaped}</bdi>")
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::configuration::tests::configuration;
    use crate::domain::draft::DraftContent;
    use crate::render::document::tests::{published, view_of};

    #[test]
    fn renders_semantic_rtl_html() {
        let html = HtmlRenderer::render_fragment(&view_of(&published()));
        assert!(html.starts_with("<article dir=\"rtl\" lang=\"he\">"));
        assert!(html.contains("<h1>תחקיר 056-2026</h1>"));
        assert!(
            html.contains("<th scope=\"col\">מס׳ זנב</th><th scope=\"col\">מס׳ קרון</th><th scope=\"col\">מקטע</th>")
        );
        assert!(html.contains("<td><bdi dir=\"ltr\">08:00</bdi></td>"));
        assert!(!html.contains("role=\"note\""), "completed investigations carry no draft notice");
    }

    #[test]
    fn escapes_every_value() {
        let mut investigation = published();
        investigation.investigation.rows[0].description = "<script>alert('x')</script>\nשורה 2".into();
        investigation.investigation.activity.name = "a & b".into();
        let html = HtmlRenderer::render_fragment(&view_of(&investigation));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;<br>שורה 2"));
        assert!(html.contains("<p>a &amp; b</p>"));
    }

    #[test]
    fn marks_drafts() {
        let html = HtmlRenderer::render_fragment(&DocumentView::from_draft(&DraftContent::default(), &configuration()));
        assert!(html.contains("<p role=\"note\"><strong>טיוטה — לא להפצה</strong></p>"));
    }
}
