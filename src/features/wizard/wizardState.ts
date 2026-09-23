// State of the "new investigation" wizard as a pure reducer. Business rules
// (validation, numbering) live in the backend; this only tracks UI progress.

import type { FieldError, ImportedLog, Investigation, InvestigationDraft, StoredInvestigation } from '../../api/types';

export type WizardStep = 'rows' | 'details' | 'preview' | 'done';

export interface WizardState {
  step: WizardStep;
  log: ImportedLog | null;
  selectedRowIds: ReadonlySet<number>;
  systemId: string;
  preliminaryCheckUrl: string;
  fieldErrors: readonly FieldError[];
  preview: Investigation | null;
  created: StoredInvestigation | null;
}

export type WizardAction =
  | { type: 'imported'; log: ImportedLog }
  | { type: 'rowToggled'; rowId: number }
  | { type: 'allRowsSet'; selected: boolean }
  | { type: 'systemChanged'; systemId: string }
  | { type: 'urlChanged'; url: string }
  | { type: 'stepRequested'; step: 'rows' | 'details' }
  | { type: 'validationFailed'; errors: readonly FieldError[] }
  | { type: 'previewed'; preview: Investigation }
  | { type: 'created'; investigation: StoredInvestigation };

export const initialWizardState: WizardState = {
  step: 'rows',
  log: null,
  selectedRowIds: new Set(),
  systemId: '',
  preliminaryCheckUrl: '',
  fieldErrors: [],
  preview: null,
  created: null,
};

const withoutErrorsFor = (errors: readonly FieldError[], field: string) => errors.filter((e) => e.field !== field);

export function wizardReducer(state: WizardState, action: WizardAction): WizardState {
  switch (action.type) {
    case 'imported':
      // Rows marked in Excel are only a starting point for the selection.
      return {
        ...state,
        step: 'rows',
        log: action.log,
        selectedRowIds: new Set(action.log.rows.filter((row) => row.highlighted).map((row) => row.id)),
        fieldErrors: withoutErrorsFor(state.fieldErrors, 'rows'),
        preview: null,
      };
    case 'rowToggled': {
      const selected = new Set(state.selectedRowIds);
      if (selected.has(action.rowId)) selected.delete(action.rowId);
      else selected.add(action.rowId);
      return { ...state, selectedRowIds: selected, fieldErrors: withoutErrorsFor(state.fieldErrors, 'rows'), preview: null };
    }
    case 'allRowsSet':
      return {
        ...state,
        selectedRowIds: new Set(action.selected ? (state.log?.rows ?? []).map((row) => row.id) : []),
        fieldErrors: withoutErrorsFor(state.fieldErrors, 'rows'),
        preview: null,
      };
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
      return { ...state, step: action.step };
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
  return state.log !== null && state.selectedRowIds.size > 0;
}

/** The draft sent to the backend, or null before a workbook is imported. */
export function toDraft(state: WizardState): InvestigationDraft | null {
  if (!state.log) return null;
  return {
    importId: state.log.importId,
    // Keep workbook order; the backend enforces it too.
    selectedRowIds: state.log.rows.filter((row) => state.selectedRowIds.has(row.id)).map((row) => row.id),
    systemId: state.systemId,
    preliminaryCheckUrl: state.preliminaryCheckUrl,
  };
}

export function errorFor(state: WizardState, field: string): FieldError | undefined {
  return state.fieldErrors.find((e) => e.field === field);
}
