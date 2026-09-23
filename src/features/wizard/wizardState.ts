// State of the "new investigation" wizard as a pure reducer. Business rules
// (parsing, validation, numbering) live in the backend; this only tracks the
// operator's rows and progress. Rows change only through explicit operator
// actions: paste, edit, remove, clear.

import type { FieldError, Investigation, InvestigationDraft, LogRow, PastedRows, StoredInvestigation } from '../../api/types';

export type WizardStep = 'rows' | 'details' | 'preview' | 'done';

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
  rows: readonly ReviewRow[];
  nextKey: number;
  lastPaste: LastPaste | null;
  systemId: string;
  preliminaryCheckUrl: string;
  fieldErrors: readonly FieldError[];
  preview: Investigation | null;
  created: StoredInvestigation | null;
}

export type WizardAction =
  | { type: 'rowsPasted'; pasted: PastedRows }
  | { type: 'rowUpdated'; key: number; row: LogRow }
  | { type: 'rowRemoved'; key: number }
  | { type: 'rowsCleared' }
  | { type: 'systemChanged'; systemId: string }
  | { type: 'urlChanged'; url: string }
  | { type: 'stepRequested'; step: 'rows' | 'details' }
  | { type: 'validationFailed'; errors: readonly FieldError[] }
  | { type: 'previewed'; preview: Investigation }
  | { type: 'created'; investigation: StoredInvestigation };

export const initialWizardState: WizardState = {
  step: 'rows',
  rows: [],
  nextKey: 1,
  lastPaste: null,
  systemId: '',
  preliminaryCheckUrl: '',
  fieldErrors: [],
  preview: null,
  created: null,
};

const withoutErrorsFor = (errors: readonly FieldError[], field: string) => errors.filter((e) => e.field !== field);

/** Any change to the rows invalidates the preview and clears row errors. */
function withRows(state: WizardState, rows: readonly ReviewRow[]): WizardState {
  return { ...state, rows, preview: null, fieldErrors: withoutErrorsFor(state.fieldErrors, 'rows') };
}

export function wizardReducer(state: WizardState, action: WizardAction): WizardState {
  switch (action.type) {
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
    case 'systemChanged':
      return {
        ...state,
        systemId: action.systemId,
        fieldErrors: withoutErrorsFor(state.fieldErrors, 'systemId'),
        preview: null,
      };
    case 'urlChanged':
      return {
        ...state,
        preliminaryCheckUrl: action.url,
        fieldErrors: withoutErrorsFor(state.fieldErrors, 'preliminaryCheckUrl'),
        preview: null,
      };
    case 'stepRequested':
      if (action.step === 'details' && !canContinueFromRows(state)) return state;
      return { ...state, step: action.step, lastPaste: null };
    case 'validationFailed': {
      // Row problems can only be fixed on the first step.
      const step = action.errors.some((e) => e.field === 'rows') ? 'rows' : state.step;
      return { ...state, step, fieldErrors: action.errors };
    }
    case 'previewed':
      return { ...state, step: 'preview', preview: action.preview, fieldErrors: [] };
    case 'created':
      return { ...state, step: 'done', created: action.investigation };
  }
}

export function canContinueFromRows(state: WizardState): boolean {
  return state.rows.length > 0;
}

/** The draft sent to the backend: exactly the rows the operator reviewed. */
export function toDraft(state: WizardState): InvestigationDraft {
  return {
    systemId: state.systemId,
    preliminaryCheckUrl: state.preliminaryCheckUrl,
    rows: state.rows.map(({ time, from, to, description }) => ({ time, from, to, description })),
  };
}

export function errorFor(state: WizardState, field: string): FieldError | undefined {
  return state.fieldErrors.find((e) => e.field === field);
}
