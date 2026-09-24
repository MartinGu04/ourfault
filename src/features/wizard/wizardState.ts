// State of the investigation wizard as a pure reducer. Business rules
// (parsing, validation, numbering) live in the backend; this only tracks
// what the operator entered and where they are. Chronology rows change only
// through explicit operator actions: paste, edit, remove, clear.

import type {
  ActivityInput,
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
  created: null,
  expectedNumber: null,
};

/** Continues a saved draft where the operator stopped. */
export function stateFromDraft(draft: Draft | null): WizardState {
  if (!draft) return initialWizardState;
  const { content } = draft;
  return {
    ...initialWizardState,
    step: draft.step,
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

const without = (errors: readonly FieldError[], fields: readonly string[]) =>
  errors.filter((error) => !fields.includes(error.field));

/** Errors of a section change position when rows are added or removed. */
const withoutSection = (errors: readonly FieldError[], sectionId: string) =>
  errors.filter((error) => error.field !== `sections.${sectionId}` && !error.field.startsWith(`sections.${sectionId}.`));

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
      return {
        ...state,
        activity: { ...state.activity, ...action.patch },
        fieldErrors: without(state.fieldErrors, keys.flatMap((key) => ACTIVITY_ERROR_FIELDS[key])),
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
      return {
        ...state,
        sections: { ...state.sections, [action.sectionId]: list },
        fieldErrors: without(state.fieldErrors, [
          prefix,
          `${prefix}.${action.index}`,
          `${prefix}.${action.index}.${action.fieldId}`,
        ]),
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
    case 'stepRequested':
      return { ...state, step: action.step, lastPaste: null };
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

export function errorsForStep(state: WizardState, step: DraftStep, stepOf: (field: string) => DraftStep): number {
  return state.fieldErrors.filter((error) => stepOf(error.field) === step).length;
}
