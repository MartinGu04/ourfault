// Hebrew names for language-neutral values from the backend. The backend
// sends codes (e.g. activity type "mission"); the UI names them here.

import type {
  ActivityStatus,
  ActivityTypeKind,
  DraftStep,
  FieldKind,
  InvestigationStatus,
  SectionDefinition,
} from '../api/types';

export const ACTIVITY_TYPES: { value: ActivityTypeKind; label: string }[] = [
  { value: 'mission', label: 'משימה' },
  { value: 'experiment', label: 'ניסוי' },
  { value: 'training', label: 'אימון' },
  { value: 'other', label: 'אחר' },
];

export const ACTIVITY_STATUSES: { value: ActivityStatus; label: string }[] = [
  { value: 'active', label: 'פעילה' },
  { value: 'completed', label: 'הסתיימה' },
];

export const activityTypeLabel = (kind: ActivityTypeKind) =>
  ACTIVITY_TYPES.find((type) => type.value === kind)?.label ?? kind;

export const INVESTIGATION_STATUS_LABELS: Record<InvestigationStatus, string> = {
  draft: 'טיוטה',
  completed: 'הושלם',
  distributed: 'הופץ',
};

export const FIELD_KIND_LABELS: Record<FieldKind, string> = {
  text: 'טקסט',
  number: 'מספר',
  boolean: 'כן / לא',
  singleSelect: 'בחירה אחת',
  multiSelect: 'בחירה מרובה',
  station: 'תחנה לוויינית',
  system: 'מערכת',
};

export const STEPS: { key: DraftStep; label: string }[] = [
  { key: 'activity', label: 'פרטי הפעילות' },
  { key: 'technical', label: 'פרטים טכניים' },
  { key: 'chronology', label: 'השתלשלות אירועים' },
  { key: 'review', label: 'בדיקות מקדימות וסקירה' },
];

export const stepLabel = (step: DraftStep) => STEPS.find((s) => s.key === step)?.label ?? step;

export const SETUP_ITEMS: Record<string, { title: string; hint: string }> = {
  systems: { title: 'מערכות', hint: 'לפחות מערכת פעילה אחת' },
  distributionLists: { title: 'רשימות תפוצה', hint: 'לכל מערכת פעילה מוגדרים נמענים' },
  stations: { title: 'תחנות לווייניות', hint: 'לפחות תחנה פעילה אחת' },
  sections: { title: 'סעיפים טכניים', hint: 'לפחות סעיף פעיל אחד' },
  mailTemplate: { title: 'תבנית דוא״ל', hint: 'נושא וגוף הודעת ההפצה' },
  publication: { title: 'יעד פרסום', hint: 'אתר ורשימה ב-SharePoint' },
};

/** Top-level form fields and the step where they are edited. */
export const FIELD_LABELS: Record<string, { label: string; step: DraftStep }> = {
  activityName: { label: 'שם משימה / פעילות', step: 'activity' },
  activityType: { label: 'סוג פעילות', step: 'activity' },
  activityTypeOther: { label: 'תיאור סוג הפעילות', step: 'activity' },
  systemIds: { label: 'מערכות מופעלות', step: 'activity' },
  activityStatus: { label: 'סטטוס פעילות', step: 'activity' },
  plannedStart: { label: 'תכנון – התחלה', step: 'activity' },
  plannedEnd: { label: 'תכנון – סיום', step: 'activity' },
  actualStart: { label: 'ביצוע בפועל – התחלה', step: 'activity' },
  actualEnd: { label: 'ביצוע בפועל – סיום', step: 'activity' },
  nightActivity: { label: 'משימת לילה', step: 'activity' },
  seniorStaffing: { label: 'איוש בכיר (נגד)', step: 'activity' },
  rows: { label: 'השתלשלות אירועים', step: 'chronology' },
  preliminaryCheckUrl: { label: 'קישור לבדיקות מקדימות', step: 'review' },
};

/**
 * Where a backend field key points: a named top-level field, or a section
 * field (`sections.{section}.{row}.{field}`), described with the names from
 * the configuration.
 */
export function describeField(
  key: string,
  sections: readonly SectionDefinition[],
): { label: string; step: DraftStep } {
  const known = FIELD_LABELS[key];
  if (known) return known;
  const [prefix, sectionId, row, fieldId] = key.split('.');
  if (prefix === 'sections' && sectionId) {
    const section = sections.find((s) => s.id === sectionId);
    const parts = [section?.name ?? 'סעיף טכני'];
    if (row !== undefined && section?.mode === 'repeating') parts.push(`שורה ${Number(row) + 1}`);
    const field = section?.fields.find((f) => f.id === fieldId);
    if (field) parts.push(field.label);
    return { label: parts.join(' · '), step: 'technical' };
  }
  return { label: key, step: 'review' };
}
