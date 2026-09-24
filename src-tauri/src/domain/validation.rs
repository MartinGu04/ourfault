//! Input validation shared by the domain. Errors are returned as stable,
//! language-neutral codes; the UI maps them to Hebrew messages.

use serde::Serialize;
use url::Url;

/// A validation failure attached to a named input field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    /// camelCase field name as used by the frontend, e.g. `preliminaryCheckUrl`.
    pub field: String,
    /// Stable machine-readable code, e.g. `unsupported_scheme`.
    pub code: &'static str,
}

impl FieldError {
    pub fn new(field: &str, code: &'static str) -> Self {
        Self { field: field.to_owned(), code }
    }
}

pub const MAX_URL_LENGTH: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlPolicy {
    /// `http` or `https`. Intranet systems may still serve plain HTTP.
    HttpOrHttps,
    HttpsOnly,
}

/// Validates the structure of a web URL and returns it trimmed.
///
/// This only inspects the text. It never resolves or fetches the URL.
pub fn validate_web_url(input: &str, policy: UrlPolicy) -> Result<String, &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("required");
    }
    if trimmed.len() > MAX_URL_LENGTH {
        return Err("too_long");
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("invalid_url");
    }
    let url = Url::parse(trimmed).map_err(|_| "invalid_url")?;
    let scheme_allowed = match policy {
        UrlPolicy::HttpOrHttps => matches!(url.scheme(), "http" | "https"),
        UrlPolicy::HttpsOnly => url.scheme() == "https",
    };
    if !scheme_allowed {
        return Err("unsupported_scheme");
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err("missing_host");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("credentials_not_allowed");
    }
    Ok(trimmed.to_owned())
}

/// Validates the preliminary-checks link pasted by the operator.
pub fn validate_preliminary_check_url(input: &str) -> Result<String, &'static str> {
    validate_web_url(input, UrlPolicy::HttpOrHttps)
}

/// Deliberately simple structural e-mail check for configured distribution
/// lists (`local@domain.tld`). Returns the trimmed, lower-cased address.
pub fn validate_email(input: &str) -> Result<String, &'static str> {
    let address = input.trim().to_lowercase();
    if address.is_empty() {
        return Err("required");
    }
    let (local, domain) = address.split_once('@').ok_or("invalid_email")?;
    let local_ok = !local.is_empty()
        && local.len() <= 64
        && local.chars().all(|c| c.is_ascii_alphanumeric() || "._%+-".contains(c));
    let labels: Vec<&str> = domain.split('.').collect();
    let domain_ok = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if address.len() > 254 || !local_ok || !domain_ok {
        return Err("invalid_email");
    }
    Ok(address)
}

/// Trims `value` and checks that it is non-empty and at most `max_chars` long.
pub fn required_text(value: &str, max_chars: usize) -> Result<String, &'static str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err("required")
    } else if trimmed.chars().count() > max_chars {
        Err("too_long")
    } else {
        Ok(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_links() {
        assert_eq!(
            validate_preliminary_check_url("  https://checks.example.com/runs/4471?view=summary ").unwrap(),
            "https://checks.example.com/runs/4471?view=summary"
        );
        assert!(validate_preliminary_check_url("http://intranet.example.com/checks/12").is_ok());
    }

    #[test]
    fn rejects_empty_and_malformed_input() {
        assert_eq!(validate_preliminary_check_url("   "), Err("required"));
        assert_eq!(validate_preliminary_check_url("checks.example.com/12"), Err("invalid_url"));
        assert_eq!(validate_preliminary_check_url("https://exa mple.com"), Err("invalid_url"));
        assert_eq!(validate_preliminary_check_url("https://"), Err("invalid_url"));
    }

    #[test]
    fn rejects_non_web_schemes() {
        for input in [
            "javascript:alert(1)",
            "file:///C:/Windows/system32",
            "data:text/html,<b>x</b>",
            "ftp://files.example.com/a",
            "mailto:ops@example.com",
        ] {
            assert_eq!(validate_preliminary_check_url(input), Err("unsupported_scheme"), "{input}");
        }
    }

    #[test]
    fn rejects_embedded_credentials() {
        assert_eq!(
            validate_preliminary_check_url("https://user:secret@checks.example.com/"),
            Err("credentials_not_allowed")
        );
    }

    #[test]
    fn rejects_overlong_urls() {
        let long = format!("https://checks.example.com/{}", "a".repeat(MAX_URL_LENGTH));
        assert_eq!(validate_preliminary_check_url(&long), Err("too_long"));
    }

    #[test]
    fn https_only_policy_rejects_http() {
        assert_eq!(validate_web_url("http://sharepoint.example.com", UrlPolicy::HttpsOnly), Err("unsupported_scheme"));
        assert!(validate_web_url("https://sharepoint.example.com/sites/alpha", UrlPolicy::HttpsOnly).is_ok());
    }

    #[test]
    fn validates_email_addresses() {
        assert_eq!(validate_email(" Alpha.Ops@Example.com ").unwrap(), "alpha.ops@example.com");
        for bad in ["", "no-at-sign", "@example.com", "a@b", "a@-x.com", "a b@example.com", "a@exa_mple.com"] {
            assert!(validate_email(bad).is_err(), "should reject {bad:?}");
        }
    }
}
