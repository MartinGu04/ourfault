// State of the investigation wizard as a pure reducer. Business rules
// (parsing, validation, numbering) live in the backend; this only tracks
// what the operator entered and where they are. Chronology rows change only
// through explicit operator actions: paste, edit, remove, clear.

import { MISSING_CODES } from '../../api/errors';
import type {
  ActivityInput,
  Advisory,
  Completion,
  Draft,
  DraftContent,
  DraftStep,
  FieldError,
  FieldValue,
  InvestigationNumber,
  LogRow,
  PastedRows,
  SectionRecord,
} from '../../api/types';
import { describeField, STEPS } from '../../lib/labels';

export type WizardStep = DraftStep | 'done';

/** A pasted row under review; `key` identifies it in the list. */
export interface ReviewRow extends LogRow {
  key: number;
}

export interface LastPaste {
  count: number;
  headerSkipped: boolean;
}

export interface WizardState {
  step: WizardStep;
  activity: ActivityInput;
  sections: Record<string, SectionRecord[]>;
  rows: readonly ReviewRow[];
  nextKey: number;
  lastPaste: LastPaste | null;
  preliminaryCheckUrl: string;
  /** Problems reported by the backend (review or completion). */
  fieldErrors: readonly FieldError[];
  /**
   * The backend's latest findings for the content being edited (errors,
   * warnings, information), shown next to their fields. Null until the
   * first result arrives.
   */
  assessment: readonly Advisory[] | null;
  /** Fields the operator has changed or left, by backend field key. */
  touched: readonly string[];
  /** Steps the operator has moved on from (or everything, once on review). */
  attempted: readonly DraftStep[];
  /** Whether the last step change went forward or back (for the transition). */
  direction: 'forward' | 'back';
  created: Completion | null;
  /** The number shown before completion, to explain if it changed. */
  expectedNumber: InvestigationNumber | null;
}

export type WizardAction =
  | { type: 'activityChanged'; patch: Partial<ActivityInput> }
  | { type: 'sectionValueChanged'; sectionId: string; index: number; fieldId: string; value: FieldValue | null }
  | { type: 'sectionRowAdded'; sectionId: string }
  | { type: 'sectionRowRemoved'; sectionId: string; index: number }
  | { type: 'rowsPasted'; pasted: PastedRows }
  | { type: 'rowUpdated'; key: number; row: LogRow }
  | { type: 'rowRemoved'; key: number }
  | { type: 'rowsCleared' }
  | { type: 'urlChanged'; url: string }
  | { type: 'stepRequested'; step: DraftStep }
  | { type: 'assessed'; advisories: readonly Advisory[] }
  | { type: 'fieldTouched'; field: string }
  | { type: 'reviewed'; issues: readonly FieldError[]; expectedNumber: InvestigationNumber | null }
  | { type: 'validationFailed'; errors: readonly FieldError[] }
  | { type: 'created'; completion: Completion };

export const emptyActivity: ActivityInput = {
  name: '',
  activityType: null,
  activityTypeOther: '',
  systemIds: [],
  status: null,
  plannedStart: '',
  plannedEnd: '',
  actualStart: '',
  actualEnd: '',
  nightActivity: null,
  seniorStaffing: null,
};

export const initialWizardState: WizardState = {
  step: 'activity',
  activity: emptyActivity,
  sections: {},
  rows: [],
  nextKey: 1,
  lastPaste: null,
  preliminaryCheckUrl: '',
  fieldErrors: [],
  assessment: null,
  touched: [],
  attempted: [],
  direction: 'forward',
  created: null,
  expectedNumber: null,
};

/** Continues a saved draft where the operator stopped. */
export function stateFromDraft(draft: Draft | null): WizardState {
  if (!draft) return initialWizardState;
  const { content } = draft;
  const resumedAt = STEPS.findIndex((step) => step.key === draft.step);
  return {
    ...initialWizardState,
    step: draft.step,
    // The operator already went through the steps before this one.
    attempted: stepsUntil(draft.step === 'review' ? STEPS.length : resumedAt),
    activity: { ...emptyActivity, ...content.activity },
    sections: content.sections,
    rows: content.rows.map((row, index) => ({ ...row, key: index + 1 })),
    nextKey: content.rows.length + 1,
    preliminaryCheckUrl: content.preliminaryCheckUrl,
  };
}

/** Backend field keys affected by each activity input. */
const ACTIVITY_ERROR_FIELDS: Record<keyof ActivityInput, string[]> = {
  name: ['activityName'],
  activityType: ['activityType', 'activityTypeOther'],
  activityTypeOther: ['activityTypeOther'],
  systemIds: ['systemIds'],
  status: ['activityStatus', 'actualEnd'],
  plannedStart: ['plannedStart', 'plannedEnd'],
  plannedEnd: ['plannedEnd'],
  actualStart: ['actualStart', 'actualEnd'],
  actualEnd: ['actualEnd'],
  nightActivity: ['nightActivity'],
  seniorStaffing: ['seniorStaffing'],
};

/** The backend field key of each activity input. */
const ACTIVITY_FIELD: Record<keyof ActivityInput, string> = {
  name: 'activityName',
  activityType: 'activityType',
  activityTypeOther: 'activityTypeOther',
  systemIds: 'systemIds',
  status: 'activityStatus',
  plannedStart: 'plannedStart',
  plannedEnd: 'plannedEnd',
  actualStart: 'actualStart',
  actualEnd: 'actualEnd',
  nightActivity: 'nightActivity',
  seniorStaffing: 'seniorStaffing',
};

const stepsUntil = (count: number): DraftStep[] => STEPS.slice(0, Math.max(count, 0)).map((step) => step.key);

const without = <T extends { field: string }>(errors: readonly T[], fields: readonly string[]) =>
  errors.filter((error) => !fields.includes(error.field));

/** Stale findings for a changed input are dropped until the backend re-assesses. */
const assessmentWithout = (assessment: readonly Advisory[] | null, fields: readonly string[]) =>
  assessment && without(assessment, fields);

const touch = (touched: readonly string[], fields: readonly string[]) =>
  fields.every((field) => touched.includes(field)) ? touched : [...new Set([...touched, ...fields])];

/** Errors of a section change position when rows are added or removed. */
const inSection = (field: string, sectionId: string) =>
  field === `sections.${sectionId}` || field.startsWith(`sections.${sectionId}.`);
const withoutSection = <T extends { field: string }>(errors: readonly T[], sectionId: string) =>
  errors.filter((error) => !inSection(error.field, sectionId));

function withRows(state: WizardState, rows: readonly ReviewRow[]): WizardState {
  return { ...state, rows, fieldErrors: without(state.fieldErrors, ['rows']) };
}

function records(state: WizardState, sectionId: string): SectionRecord[] {
  return state.sections[sectionId] ?? [];
}

export function wizardReducer(state: WizardState, action: WizardAction): WizardState {
  switch (action.type) {
    case 'activityChanged': {
      const keys = Object.keys(action.patch) as (keyof ActivityInput)[];
      const affected = keys.flatMap((key) => ACTIVITY_ERROR_FIELDS[key]);
      return {
        ...state,
        activity: { ...state.activity, ...action.patch },
        fieldErrors: without(state.fieldErrors, affected),
        assessment: assessmentWithout(state.assessment, affected),
        touched: touch(state.touched, keys.map((key) => ACTIVITY_FIELD[key])),
      };
    }
    case 'sectionValueChanged': {
      const current = records(state, action.sectionId);
      // A single-record section may have no record yet.
      const list = current.length > action.index ? [...current] : [...current, {}];
      const record = { ...list[action.index] };
      if (action.value === null) delete record[action.fieldId];
      else record[action.fieldId] = action.value;
      list[action.index] = record;
      const prefix = `sections.${action.sectionId}`;
      const affected = [prefix, `${prefix}.${action.index}`, `${prefix}.${action.index}.${action.fieldId}`];
      return {
        ...state,
        sections: { ...state.sections, [action.sectionId]: list },
        fieldErrors: without(state.fieldErrors, affected),
        assessment: assessmentWithout(state.assessment, affected),
        touched: touch(state.touched, [`${prefix}.${action.index}.${action.fieldId}`]),
      };
    }
    case 'sectionRowAdded':
      return {
        ...state,
        sections: { ...state.sections, [action.sectionId]: [...records(state, action.sectionId), {}] },
      };
    case 'sectionRowRemoved':
      return {
        ...state,
        sections: {
          ...state.sections,
          [action.sectionId]: records(state, action.sectionId).filter((_, index) => index !== action.index),
        },
        fieldErrors: withoutSection(state.fieldErrors, action.sectionId),
        assessment: state.assessment && withoutSection(state.assessment, action.sectionId),
        touched: state.touched.filter((field) => !inSection(field, action.sectionId)),
      };
    case 'rowsPasted': {
      // Pasting again appends, so rows can be collected from several places.
      const added = action.pasted.rows.map((row, index) => ({ ...row, key: state.nextKey + index }));
      return {
        ...withRows(state, [...state.rows, ...added]),
        nextKey: state.nextKey + added.length,
        lastPaste: { count: added.length, headerSkipped: action.pasted.headerSkipped },
      };
    }
    case 'rowUpdated':
      return withRows(
        state,
        state.rows.map((row) => (row.key === action.key ? { ...action.row, key: row.key } : row)),
      );
    case 'rowRemoved':
      return { ...withRows(state, state.rows.filter((row) => row.key !== action.key)), lastPaste: null };
    case 'rowsCleared':
      return { ...withRows(state, []), lastPaste: null };
    case 'urlChanged':
      return {
        ...state,
        preliminaryCheckUrl: action.url,
        fieldErrors: without(state.fieldErrors, ['preliminaryCheckUrl']),
      };
    case 'stepRequested': {
      // Leaving a step reveals what is missing there; reaching the review
      // reveals everything.
      const left = state.step === 'done' || state.step === action.step ? [] : [state.step];
      const attempted =
        action.step === 'review' ? stepsUntil(STEPS.length) : [...new Set([...state.attempted, ...left])];
      const from = STEPS.findIndex((step) => step.key === state.step);
      const to = STEPS.findIndex((step) => step.key === action.step);
      const direction = to < from ? 'back' : 'forward';
      return { ...state, step: action.step, lastPaste: null, attempted, direction };
    }
    case 'assessed':
      return { ...state, assessment: action.advisories };
    case 'fieldTouched':
      return { ...state, touched: touch(state.touched, [action.field]) };
    case 'reviewed':
    case 'validationFailed': {
      const errors = action.type === 'reviewed' ? action.issues : action.errors;
      const expectedNumber = action.type === 'reviewed' ? action.expectedNumber : state.expectedNumber;
      return { ...state, fieldErrors: errors, expectedNumber };
    }
    case 'created':
      return { ...state, step: 'done', created: action.completion };
  }
}

/** The content saved in the draft: exactly what the operator entered. */
export function toContent(state: WizardState): DraftContent {
  return {
    activity: state.activity,
    sections: state.sections,
    rows: state.rows.map(({ time, from, to, description }) => ({ time, from, to, description })),
    preliminaryCheckUrl: state.preliminaryCheckUrl,
  };
}

export function errorFor(state: WizardState, field: string): FieldError | undefined {
  return state.fieldErrors.find((e) => e.field === field);
}

/** Codes about something absent: shown only once the operator got to it. */
const ABSENT_CODES: readonly string[] = [...MISSING_CODES, 'empty_row'];

export const stepOfField = (field: string): DraftStep => describeField(field, []).step;

/**
 * The errors to show next to fields while editing. Errors the backend
 * reported on review or completion are always shown. Of the current
 * assessment, an invalid value shows at once; a missing value shows only
 * after the operator changed or left that field, or moved on from its step,
 * so a new draft does not open as a wall of red.
 */
export function visibleErrors(state: WizardState): FieldError[] {
  const shown: FieldError[] = [...state.fieldErrors];
  for (const advisory of state.assessment ?? []) {
    if (advisory.severity !== 'error') continue;
    const revealed =
      !ABSENT_CODES.includes(advisory.code) ||
      state.touched.includes(advisory.field) ||
      state.attempted.includes(stepOfField(advisory.field));
    const duplicate = shown.some((error) => error.field === advisory.field && error.code === advisory.code);
    if (revealed && !duplicate) shown.push({ field: advisory.field, code: advisory.code });
  }
  return shown;
}

/** Errors block completion; warnings and information never do. */
export function blockingIssues(advisories: readonly Advisory[]): FieldError[] {
  return advisories.filter((advisory) => advisory.severity === 'error').map(({ field, code }) => ({ field, code }));
}

/** Non-blocking findings (warnings and information) about these fields. */
export function notesFor(state: WizardState, fields: readonly string[]): Advisory[] {
  return (state.assessment ?? []).filter((advisory) => advisory.severity !== 'error' && fields.includes(advisory.field));
}

/** What the last paste added, e.g. "נוספו 7 שורות. סה״כ 12 שורות בתחקיר." */
export function pasteFeedback(lastPaste: LastPaste, total: number): string {
  const added = lastPaste.count === 1 ? 'נוספה שורה אחת' : `נוספו ${lastPaste.count.toLocaleString('he-IL')} שורות`;
  const header = lastPaste.headerSkipped ? ' (שורת הכותרות הושמטה)' : '';
  const sum = total === 1 ? 'שורה אחת' : `${total.toLocaleString('he-IL')} שורות`;
  return `${added}${header}. סה״כ ${sum} בתחקיר.`;
}

export function errorsForStep(state: WizardState, step: DraftStep, stepOf: (field: string) => DraftStep): number {
  return state.fieldErrors.filter((error) => stepOf(error.field) === step).length;
}
