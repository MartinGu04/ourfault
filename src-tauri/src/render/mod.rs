//! Rendering: output formats derived from the structured investigation.
//!
//! ```text
//! Investigation / Draft ──▶ DocumentView ──▶ HtmlRenderer (SharePoint rich text)
//!                                        └─▶ PdfRenderer  (export, shared folder)
//! ```
//!
//! Renderers are pure functions without I/O. Publishers and the export
//! service decide which output they need.

pub mod document;
pub mod html;
pub(crate) mod labels;
pub mod pdf;
