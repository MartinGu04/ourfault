//! Hebrew wording of generated documents (preview, HTML, PDF). The domain
//! model stays language-neutral; this is the only place that names things
//! for readers of the document.

use crate::domain::activity::{ActivityStatus, ActivityTypeKind};
use crate::domain::lifecycle::InvestigationStatus;

pub const INVESTIGATION: &str = "תחקיר";
pub const DRAFT_TITLE: &str = "טיוטת תחקיר";
pub const DRAFT_NOTICE: &str = "טיוטה — לא להפצה";
pub const MISSING: &str = "—";
pub const YES: &str = "כן";
pub const NO: &str = "לא";

pub const ACTIVITY_DETAILS: &str = "פרטי הפעילות";
pub const ACTIVITY_NAME: &str = "שם משימה / פעילות";
pub const ACTIVITY_TYPE: &str = "סוג פעילות";
pub const SYSTEMS: &str = "מערכות מופעלות";
pub const ACTIVITY_STATUS: &str = "סטטוס פעילות";
pub const NIGHT_ACTIVITY: &str = "משימת לילה";
pub const SENIOR_STAFFING: &str = "איוש בכיר (נגד)";

pub const PLANNED: &str = "תכנון";
pub const ACTUAL: &str = "ביצוע בפועל";
pub const START: &str = "התחלה";
pub const END: &str = "סיום";
pub const NOT_ENDED: &str = "טרם הסתיימה";

pub const CHRONOLOGY: &str = "השתלשלות אירועים";
pub const TIME: &str = "שעה";
pub const FROM: &str = "ממי";
pub const TO: &str = "למי";
pub const DESCRIPTION: &str = "תוכן";
pub const NO_ROWS: &str = "אין שורות";

pub const CHECKS: &str = "בדיקות מקדימות";
pub const CHECKS_LINK: &str = "קישור לבדיקות מקדימות";

pub const INVESTIGATION_STATUS: &str = "סטטוס התחקיר";
pub const COMPLETED_AT: &str = "הושלם בתאריך";
pub const DISTRIBUTED_AT: &str = "הופץ לראשונה בתאריך";

pub const UNNAMED: &str = "ללא שם";
pub const DRAFT_FILE_PREFIX: &str = "טיוטה";

pub const PAGE_OF: (&str, &str) = ("עמוד", "מתוך");

pub fn activity_type(kind: ActivityTypeKind) -> &'static str {
    match kind {
        ActivityTypeKind::Mission => "משימה",
        ActivityTypeKind::Experiment => "ניסוי",
        ActivityTypeKind::Training => "אימון",
        ActivityTypeKind::Other => "אחר",
    }
}

pub fn activity_status(status: ActivityStatus) -> &'static str {
    match status {
        ActivityStatus::Active => "פעילה",
        ActivityStatus::Completed => "הסתיימה",
    }
}

pub fn investigation_status(status: InvestigationStatus) -> &'static str {
    match status {
        InvestigationStatus::Draft => "טיוטה",
        InvestigationStatus::Completed => "הושלם",
        InvestigationStatus::Distributed => "הופץ",
    }
}
