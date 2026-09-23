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
    case 'import': {
      if (typeof value.code !== 'string') return { kind: 'internal' };
      const missing = Array.isArray(value.missingColumns)
        ? value.missingColumns.filter((c): c is string => typeof c === 'string')
        : [];
      return missing.length > 0
        ? { kind: 'import', code: value.code, missingColumns: missing }
        : { kind: 'import', code: value.code };
    }
    case 'notFound':
    case 'forbidden':
    case 'internal':
      return { kind: value.kind };
    default:
      return { kind: 'internal' };
  }
}

/** Isolates left-to-right text (URLs, file types) inside a Hebrew sentence. */
const ltr = (text: string) => `\u2066${text}\u2069`;

const COLUMN_NAMES: Record<string, string> = {
  time: `שעה (${ltr('Time')})`,
  from: `ממי (${ltr('From')})`,
  to: `למי (${ltr('To')})`,
  description: `תוכן (${ltr('Description')})`,
};

const IMPORT_MESSAGES: Record<string, string> = {
  file_too_large: `הקובץ גדול מדי. ניתן לייבא קבצים בגודל של עד ${ltr('10 MB')}.`,
  unreadable_workbook: `לא ניתן לקרוא את הקובץ. ודאו שמדובר בקובץ Excel תקין בפורמט ${ltr('.xlsx')}.`,
  unsupported_file_type: `ניתן לייבא קובצי Excel בפורמט ${ltr('.xlsx')} בלבד.`,
  file_locked: 'אין הרשאה לקרוא את הקובץ או שהוא נעול. סגרו אותו ב-Excel ונסו שוב.',
  file_unreadable: 'לא ניתן לפתוח את הקובץ.',
  no_worksheet: 'לא נמצאו גיליונות בקובץ.',
  no_rows: 'לא נמצאו שורות נתונים בגיליון הראשון.',
  too_many_rows: 'הגיליון מכיל יותר מ-5,000 שורות. יש לפצל את היומן לקבצים קטנים יותר.',
};

const FIELD_MESSAGES: Record<string, string> = {
  required: 'שדה חובה',
  too_long: 'הערך ארוך מדי',
  too_many: 'יותר מדי ערכים ברשימה',
  invalid_url: `יש להזין כתובת מלאה ותקינה, לדוגמה ${ltr('https://checks.example.com/runs/1')}`,
  unsupported_scheme: `הכתובת צריכה להתחיל ב-${ltr('https://')} או ב-${ltr('http://')}`,
  missing_host: 'בכתובת חסר שם שרת',
  credentials_not_allowed: 'אין לכלול שם משתמש או סיסמה בכתובת',
  system_inactive: 'המערכת שנבחרה אינה פעילה',
  no_rows_selected: 'יש לבחור לפחות שורה אחת',
  too_many_rows: 'ניתן לכלול עד 500 שורות בתחקיר',
  unknown_row: 'חלק מהשורות שנבחרו אינן קיימות עוד. יש לייבא את הקובץ מחדש.',
  import_expired: 'הקובץ שיובא אינו זמין עוד. יש לייבא אותו מחדש.',
  invalid_email: 'אחת הכתובות אינה כתובת דוא״ל תקינה',
  invalid_characters: `השם מכיל תווים שאינם מותרים: ${ltr('/ \\ : * ? " < > | # %')}`,
  duplicate_name: 'כבר קיימת מערכת בשם זה',
  invalid_number: `מספר תחקיר לא תקין. יש להזין מספר כמו ${ltr('056-2026')}`,
};

/** Field-specific wording where the generic message would be misleading. */
const FIELD_OVERRIDES: Record<string, Record<string, string>> = {
  sharepointSiteUrl: { unsupported_scheme: `כתובת האתר חייבת להתחיל ב-${ltr('https://')}` },
  systemId: { required: 'יש לבחור מערכת' },
};

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
    case 'import': {
      if (appError.code === 'missing_columns') {
        const names = (appError.missingColumns ?? []).map((c) => COLUMN_NAMES[c] ?? c).join(', ');
        return `לא נמצאו בקובץ העמודות הנדרשות: ${names}. ודאו ששורת הכותרות נמצאת בראש הגיליון הראשון.`;
      }
      return IMPORT_MESSAGES[appError.code] ?? 'לא ניתן לייבא את הקובץ.';
    }
    case 'notFound':
      return 'הפריט המבוקש לא נמצא.';
    case 'forbidden':
      return 'אין לך הרשאה לבצע פעולה זו.';
    case 'internal':
      return 'אירעה שגיאה בלתי צפויה. נסו שוב, ואם הבעיה חוזרת פנו לתמיכה.';
  }
}
