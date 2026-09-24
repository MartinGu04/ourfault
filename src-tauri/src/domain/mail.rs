//! Distribution e-mail: one administrator-configured subject/body template
//! for all investigations, with a small fixed set of placeholders, and the
//! recipients of every system involved in the activity.
//!
//! The sender and signature are deliberately absent: the real distribution
//! adapter sends as the signed-in Outlook user, whose own signature applies.

use serde::{Deserialize, Serialize};

use super::system::System;
use super::validation::{required_text, FieldError};

pub const PLACEHOLDER_ACTIVITY_NAME: &str = "ACTIVITY_NAME";
pub const PLACEHOLDER_SYSTEMS: &str = "SYSTEMS";
pub const PLACEHOLDER_NUMBER: &str = "INVESTIGATION_NUMBER";
pub const PLACEHOLDER_LINK: &str = "INVESTIGATION_LINK";
pub const PLACEHOLDERS: [&str; 4] =
    [PLACEHOLDER_ACTIVITY_NAME, PLACEHOLDER_SYSTEMS, PLACEHOLDER_NUMBER, PLACEHOLDER_LINK];

pub const MAX_SUBJECT_CHARS: usize = 200;
pub const MAX_BODY_CHARS: usize = 4000;
pub const MAX_LINK_TEXT_CHARS: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailTemplate {
    /// Single line. Placeholders: see [`PLACEHOLDERS`].
    pub subject: String,
    /// Plain text with placeholders. `{{INVESTIGATION_LINK}}` becomes a
    /// hyperlink whose text is `link_text`.
    pub body: String,
    pub link_text: String,
}

/// The values placeholders are replaced with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailValues {
    pub activity_name: String,
    pub systems: Vec<String>,
    pub number: String,
    pub link: String,
}

/// A message ready to hand to a distribution adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistributionMessage {
    pub to: Vec<String>,
    pub subject: String,
    /// Plain-text body; the link is written out as text.
    pub body_text: String,
    /// HTML body (escaped, right-to-left) with the investigation hyperlink.
    /// For the mail client only; never rendered by OurFault.
    pub body_html: String,
}

/// Confirmation returned by a distribution adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistributionReceipt {
    pub message_id: String,
    /// RFC 3339 local timestamp.
    pub sent_at: String,
    pub recipient_count: usize,
}

enum Part<'a> {
    Text(&'a str),
    Placeholder(&'a str),
}

/// Splits a template into literal text and `{{NAME}}` placeholders.
fn parse(template: &str) -> Result<Vec<Part<'_>>, &'static str> {
    let mut parts = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        parts.push(Part::Text(&rest[..start]));
        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or("unclosed_placeholder")?;
        let name = after[..end].trim();
        if !PLACEHOLDERS.contains(&name) {
            return Err("unknown_placeholder");
        }
        parts.push(Part::Placeholder(name));
        rest = &after[end + 2..];
    }
    parts.push(Part::Text(rest));
    Ok(parts)
}

impl MailTemplate {
    /// Validates administrator input and returns the normalised template.
    pub fn validated(&self) -> Result<MailTemplate, Vec<FieldError>> {
        let mut errors = Vec::new();
        let mut check = |field: &str, result: Result<String, &'static str>| {
            result.unwrap_or_else(|code| {
                errors.push(FieldError::new(field, code));
                String::new()
            })
        };
        let subject = check(
            "subject",
            required_text(&self.subject, MAX_SUBJECT_CHARS).and_then(|subject| {
                if subject.contains(['\n', '\r']) {
                    Err("single_line")
                } else {
                    parse(&subject).map(|_| subject.clone())
                }
            }),
        );
        let body = check(
            "body",
            required_text(&self.body.replace("\r\n", "\n"), MAX_BODY_CHARS)
                .and_then(|body| parse(&body).map(|_| body.clone())),
        );
        let link_text = check("linkText", required_text(&self.link_text, MAX_LINK_TEXT_CHARS));
        if errors.is_empty() {
            Ok(MailTemplate { subject, body, link_text })
        } else {
            Err(errors)
        }
    }

    /// Fills the template. Values are inserted as text (HTML-escaped in the
    /// HTML body), never interpreted as template syntax.
    pub fn render(&self, values: &MailValues, to: Vec<String>) -> DistributionMessage {
        let systems = values.systems.join(", ");
        let value = |name: &str| match name {
            PLACEHOLDER_ACTIVITY_NAME => values.activity_name.as_str(),
            PLACEHOLDER_SYSTEMS => systems.as_str(),
            PLACEHOLDER_NUMBER => values.number.as_str(),
            _ => values.link.as_str(),
        };
        // Stored templates are validated; an invalid one renders literally.
        let parts = |template| parse(template).unwrap_or_else(|_| vec![Part::Text(template)]);

        let subject: String = parts(&self.subject)
            .iter()
            .map(|part| match part {
                Part::Text(text) => *text,
                Part::Placeholder(name) => value(name),
            })
            .collect::<String>()
            .replace(['\n', '\r'], " ");

        let mut body_text = String::new();
        let mut body_html = String::from("<div dir=\"rtl\" lang=\"he\">");
        for part in parts(&self.body) {
            match part {
                Part::Text(text) => {
                    body_text.push_str(text);
                    body_html.push_str(&escape_html(text).replace('\n', "<br>\n"));
                }
                Part::Placeholder(PLACEHOLDER_LINK) => {
                    body_text.push_str(&format!("{}: {}", self.link_text, values.link));
                    body_html.push_str(&format!(
                        "<a href=\"{}\">{}</a>",
                        escape_html(&values.link),
                        escape_html(&self.link_text)
                    ));
                }
                Part::Placeholder(name) => {
                    body_text.push_str(value(name));
                    body_html.push_str(&escape_html(value(name)));
                }
            }
        }
        body_html.push_str("</div>");
        DistributionMessage { to, subject, body_text, body_html }
    }
}

/// Recipients of all given systems: their union, duplicates removed, in the
/// order the systems (and their lists) are given.
pub fn union_recipients<'a>(systems: impl IntoIterator<Item = &'a System>) -> Vec<String> {
    let mut recipients: Vec<String> = Vec::new();
    for address in systems.into_iter().flat_map(|system| &system.distribution_list) {
        let address = address.trim().to_lowercase();
        if !address.is_empty() && !recipients.contains(&address) {
            recipients.push(address);
        }
    }
    recipients
}

pub fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::domain::system::tests::system;

    pub(crate) fn template() -> MailTemplate {
        MailTemplate {
            subject: "תחקיר למשימה \"{{ACTIVITY_NAME}}\" - {{SYSTEMS}}".into(),
            body: "היי,\n\nמצורף בזאת תחקיר למשימת \"{{ACTIVITY_NAME}}\" על מערכת/מערכות \"{{SYSTEMS}}\".\n\n{{INVESTIGATION_LINK}}\n\nבברכה,".into(),
            link_text: "תחקיר".into(),
        }
    }

    fn values() -> MailValues {
        MailValues {
            activity_name: "בלט <רומני>".into(),
            systems: vec!["מערכת אלפא".into(), "מערכת בטא".into()],
            number: "056-2026".into(),
            link: "https://sharepoint.example.com/x?ID=1&a=b".into(),
        }
    }

    #[test]
    fn fills_subject_and_both_bodies() {
        let message = template().render(&values(), vec!["a@example.com".into()]);
        assert_eq!(message.subject, "תחקיר למשימה \"בלט <רומני>\" - מערכת אלפא, מערכת בטא");
        assert!(message.body_text.contains("על מערכת/מערכות \"מערכת אלפא, מערכת בטא\"."));
        assert!(message.body_text.contains("תחקיר: https://sharepoint.example.com/x?ID=1&a=b"));
        assert!(message.body_html.starts_with("<div dir=\"rtl\" lang=\"he\">"));
        assert!(message.body_html.contains("<a href=\"https://sharepoint.example.com/x?ID=1&amp;a=b\">תחקיר</a>"));
        assert!(message.body_html.contains("בלט &lt;רומני&gt;"), "values are escaped");
        assert!(!message.body_text.contains("{{"), "no signature or placeholder is left behind");
    }

    #[test]
    fn values_are_not_interpreted_as_placeholders() {
        let mut values = values();
        values.activity_name = "{{INVESTIGATION_LINK}}".into();
        let message = template().render(&values, vec![]);
        assert!(message.subject.contains("\"{{INVESTIGATION_LINK}}\""));
    }

    #[test]
    fn validation_rejects_unknown_placeholders_and_multiline_subjects() {
        let mut bad = template();
        bad.subject = "{{ACTIVITY}}".into();
        bad.body = "{{SYSTEMS".into();
        bad.link_text = " ".into();
        let codes: Vec<(String, &str)> = bad.validated().unwrap_err().into_iter().map(|e| (e.field, e.code)).collect();
        assert_eq!(
            codes,
            vec![
                ("subject".into(), "unknown_placeholder"),
                ("body".into(), "unclosed_placeholder"),
                ("linkText".into(), "required")
            ]
        );
        bad = template();
        bad.subject = "a\nb".into();
        assert_eq!(bad.validated().unwrap_err(), vec![FieldError::new("subject", "single_line")]);
        assert_eq!(template().validated().unwrap(), template());
    }

    #[test]
    fn recipients_are_the_deduplicated_union_of_all_systems() {
        let mut alpha = system("alpha", true);
        alpha.distribution_list = vec!["ops@example.com".into(), "alpha@example.com".into()];
        let mut beta = system("beta", true);
        beta.distribution_list = vec!["beta@example.com".into(), "OPS@example.com".into()];
        let quiet = System { distribution_list: vec![], ..system("quiet", true) };
        assert_eq!(
            union_recipients([&alpha, &quiet, &beta]),
            vec!["ops@example.com", "alpha@example.com", "beta@example.com"]
        );
    }
}
