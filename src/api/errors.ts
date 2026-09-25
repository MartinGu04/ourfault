// Backend errors arrive as stable codes. This module validates their shape and
// turns them into Hebrew messages; technical details never reach the UI.

import type { AppError, FieldError } from './types';

export class ApiError extends Error {
  constructor(readonly appError: AppError) {
    super(`OurFault API error: ${appError.kind}`);
    this.name = 'ApiError';
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isFieldError(value: unknown): value is FieldError {
  return isRecord(value) && typeof value.field === 'string' && typeof value.code === 'string';
}

/** Narrows anything thrown by `invoke` to a known AppError. */
export function toAppError(value: unknown): AppError {
  if (!isRecord(value)) return { kind: 'internal' };
  switch (value.kind) {
    case 'validation':
      return Array.isArray(value.errors) && value.errors.every(isFieldError)
        ? { kind: 'validation', errors: value.errors }
        : { kind: 'internal' };
    case 'paste':
    case 'conflict':
      return typeof value.code === 'string' ? { kind: value.kind, code: value.code } : { kind: 'internal' };
    case 'notFound':
    case 'unavailable':
    case 'forbidden':
    case 'internal':
      return { kind: value.kind };
    default:
      return { kind: 'internal' };
  }
}

/** Isolates left-to-right text (URLs, examples) inside a Hebrew sentence. */
const ltr = (text: string) => `\u2066${text}\u2069`;

const NOT_TABULAR = 'לא זוהה מידע טבלאי מיומן המבצעים. יש להעתיק שורות מ-Excel ולהדביק אותן כאן.';
export const PASTE_TOO_LARGE_MESSAGE = 'הטקסט שהודבק גדול מדי (עד 2MB בכל הדבקה). הדביקו את השורות בכמה חלקים.';

const PASTE_MESSAGES: Record<string, string> = {
  empty_paste: NOT_TABULAR,
  too_few_columns: `${NOT_TABULAR} נדרשות לפחות ארבע עמודות: שעה, ממי, למי, תוכן.`,
  not_tabular: NOT_TABULAR,
  too_many_columns: 'בטקסט שהודבק יותר מ-64 עמודות. סמנו ב-Excel רק את עמודות היומן.',
  paste_too_large: PASTE_TOO_LARGE_MESSAGE,
  too_many_rows: 'ניתן להדביק עד 2,000 שורות.',
  cell_too_long: 'אחד התאים שהודבקו ארוך מ-4,000 תווים.',
};

const FIELD_MESSAGES: Record<string, string> = {
  required: 'שדה חובה',
  too_long: 'הערך ארוך מדי',
  too_many: 'יותר מדי ערכים ברשימה',
  invalid_url: `יש להזין כתובת מלאה ותקינה, לדוגמה ${ltr('https://checks.example.com/runs/1')}`,
  unsupported_scheme: `הכתובת צריכה להתחיל ב-${ltr('https://')} או ב-${ltr('http://')}`,
  missing_host: 'בכתובת חסר שם שרת',
  credentials_not_allowed: 'אין לכלול שם משתמש או סיסמה בכתובת',
  system_inactive: 'אחת המערכות שנבחרו אינה פעילה',
  unknown_system: 'אחת המערכות שנבחרו אינה קיימת',
  invalid_datetime: 'יש להזין תאריך ושעה תקינים',
  end_before_start: 'זמן הסיום מוקדם מזמן ההתחלה',
  invalid_value: 'ערך לא תקין',
  unknown_option: 'הערך אינו מופיע ברשימת האפשרויות',
  unknown_station: 'התחנה שנבחרה אינה קיימת',
  station_inactive: 'התחנה שנבחרה אינה פעילה. בחרו תחנה אחרת.',
  system_not_involved: 'יש לבחור אחת מהמערכות המופעלות בפעילות',
  single_record: 'בסעיף זה יש רשומה אחת בלבד',
  unknown_field: 'השדה אינו קיים עוד. רעננו את המסך ונסו שוב.',
  duplicate_label: 'כבר קיים שדה בשם זה בסעיף',
  unknown_placeholder: 'התבנית מכילה שדה לא מוכר. ניתן להשתמש רק בשדות שברשימה.',
  unclosed_placeholder: `שדה בתבנית לא נסגר. כל שדה נכתב כך: ${ltr('{{ACTIVITY_NAME}}')}`,
  single_line: 'הנושא חייב להיות בשורה אחת',
  no_recipients: 'לא הוגדרו נמענים לאף אחת מהמערכות בתחקיר. מנהל צריך להגדיר רשימת תפוצה.',
  confirmation_required: 'יש לאשר את ייצוא הטיוטה',
  draft_too_large: 'הטיוטה גדולה מדי',
  invalid_draft_id: 'הטיוטה המבוקשת לא נמצאה',
  no_rows: 'יש להדביק לפחות שורה אחת מיומן המבצעים',
  too_many_rows: 'ניתן לכלול עד 2,000 שורות בתחקיר',
  row_too_long: 'אחת השורות ארוכה מדי (עד 4,000 תווים בכל שדה)',
  empty_row: 'אחת השורות ריקה. מלאו אותה או הסירו אותה.',
  invalid_email: 'אחת הכתובות אינה כתובת דוא״ל תקינה',
  invalid_characters: `השם מכיל תווים שאינם מותרים: ${ltr('/ \\ : * ? " < > | # %')}`,
  duplicate_name: 'השם כבר קיים',
  invalid_number: 'יש להזין מספר, לדוגמה 12 או 3.5',
};

/** Field-specific wording where the generic message would be misleading. */
const FIELD_OVERRIDES: Record<string, Record<string, string>> = {
  sharepointSiteUrl: { unsupported_scheme: `כתובת האתר חייבת להתחיל ב-${ltr('https://')}` },
  systemIds: { required: 'יש לבחור לפחות מערכת אחת' },
  activityType: { required: 'יש לבחור סוג פעילות' },
  activityStatus: { required: 'יש לבחור סטטוס פעילות' },
  nightActivity: { required: 'יש לסמן כן או לא' },
  seniorStaffing: { required: 'יש לסמן כן או לא' },
  plannedEnd: { end_before_start: 'סיום התכנון מוקדם מההתחלה המתוכננת' },
  actualEnd: { end_before_start: 'הסיום בפועל מוקדם מההתחלה בפועל' },
  number: { invalid_number: `מספר תחקיר לא תקין. יש להזין מספר כמו ${ltr('056-2026')}` },
};

/** Wording of the non-blocking advisories (warnings and information). */
const ADVISORY_MESSAGES: Record<string, string> = {
  night_overlap_not_marked: 'הפעילות כוללת זמן בטווח שעות הלילה שהוגדר, אך סומנה כמשימת לילה: לא. מומלץ לבדוק.',
  actual_started_before_plan: 'הביצוע בפועל החל לפני מועד ההתחלה המתוכנן.',
  actual_started_after_plan: 'הביצוע בפועל החל לאחר מועד ההתחלה המתוכנן.',
  actual_ended_after_plan: 'הביצוע בפועל הסתיים לאחר מועד הסיום המתוכנן.',
};

export function advisoryMessage(advisory: { severity: string; field: string; code: string }): string {
  return advisory.severity === 'error'
    ? fieldMessage(advisory)
    : (ADVISORY_MESSAGES[advisory.code] ?? 'יש לבדוק ערך זה');
}

/** Codes that mean "a required value is missing" (as opposed to an invalid value). */
export const MISSING_CODES: readonly string[] = ['required', 'no_rows'];

export function fieldMessage(error: FieldError): string {
  return FIELD_OVERRIDES[error.field]?.[error.code] ?? FIELD_MESSAGES[error.code] ?? 'ערך לא תקין';
}

/** Validation messages keyed by field name (first error per field). */
export function fieldErrors(error: unknown): Record<string, string> {
  if (!(error instanceof ApiError) || error.appError.kind !== 'validation') return {};
  const result: Record<string, string> = {};
  for (const fieldError of error.appError.errors) {
    result[fieldError.field] ??= fieldMessage(fieldError);
  }
  return result;
}

/** A single user-facing sentence for any error. */
export function errorMessage(error: unknown): string {
  const appError: AppError = error instanceof ApiError ? error.appError : { kind: 'internal' };
  switch (appError.kind) {
    case 'validation':
      return appError.errors[0] ? fieldMessage(appError.errors[0]) : 'חלק מהנתונים אינם תקינים.';
    case 'paste':
      return PASTE_MESSAGES[appError.code] ?? 'לא ניתן לקרוא את הטקסט שהודבק.';
    case 'notFound':
      return 'הפריט המבוקש לא נמצא.';
    case 'conflict':
      return appError.code === 'draft_converted'
        ? 'הטיוטה כבר הפכה לתחקיר ואינה ניתנת לעריכה.'
        : 'הנתונים עודכנו בינתיים בעמדה אחרת. רעננו את המסך ונסו שוב.';
    case 'unavailable':
      return 'לא ניתן להתחבר ליעד הפרסום. התחקיר לא נוצר ולא הוקצה לו מספר. הטיוטה שמורה – נסו שוב מאוחר יותר.';
    case 'forbidden':
      return 'פעולה זו זמינה במצב מנהל בלבד.';
    case 'internal':
      return 'אירעה שגיאה בלתי צפויה. נסו שוב, ואם הבעיה חוזרת פנו לתמיכה.';
  }
}
