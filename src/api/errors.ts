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
      return typeof value.code === 'string' ? { kind: 'paste', code: value.code } : { kind: 'internal' };
    case 'notFound':
    case 'forbidden':
    case 'internal':
      return { kind: value.kind };
    default:
      return { kind: 'internal' };
  }
}

/** Isolates left-to-right text (URLs, examples) inside a Hebrew sentence. */
const ltr = (text: string) => `\u2066${text}\u2069`;

const PASTE_MESSAGES: Record<string, string> = {
  empty_paste: 'לא נמצאו שורות בטקסט שהודבק. העתיקו שורות מיומן המבצעים ב-Excel ונסו שוב.',
  paste_too_large: 'הטקסט שהודבק גדול מדי.',
  too_few_columns:
    'הטקסט שהודבק אינו נראה כמו שורות מיומן המבצעים. נדרשות לפחות ארבע עמודות: שעה, ממי, למי, תוכן. העתיקו שורות שלמות מ-Excel.',
  too_many_rows: 'ניתן להדביק עד 500 שורות לתחקיר.',
  cell_too_long: 'אחד התאים שהודבקו ארוך מ-2,000 תווים.',
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
  no_rows: 'יש להדביק לפחות שורה אחת מיומן המבצעים',
  too_many_rows: 'ניתן לכלול עד 500 שורות בתחקיר',
  row_too_long: 'אחת השורות ארוכה מדי (עד 2,000 תווים בכל שדה)',
  empty_row: 'אחת השורות ריקה. מלאו אותה או הסירו אותה.',
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
    case 'paste':
      return PASTE_MESSAGES[appError.code] ?? 'לא ניתן לקרוא את הטקסט שהודבק.';
    case 'notFound':
      return 'הפריט המבוקש לא נמצא.';
    case 'forbidden':
      return 'אין לך הרשאה לבצע פעולה זו.';
    case 'internal':
      return 'אירעה שגיאה בלתי צפויה. נסו שוב, ואם הבעיה חוזרת פנו לתמיכה.';
  }
}
