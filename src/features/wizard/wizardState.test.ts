import { describe, expect, it } from 'vitest';

import type { ImportedLog, OperationsLogRow } from '../../api/types';
import { canContinueFromRows, initialWizardState, toDraft, wizardReducer, type WizardState } from './wizardState';

function row(id: number, highlighted = false): OperationsLogRow {
  return { id, time: '08:00', from: 'א', to: 'ב', description: `שורה ${id}`, eventType: 'שגרה', highlighted };
}

const log: ImportedLog = {
  importId: 7,
  sourceFileName: 'log.xlsx',
  sheetName: 'יומן',
  rows: [row(3), row(4, true), row(5), row(6, true)],
  highlightDetectionAvailable: true,
};

const imported = (): WizardState => wizardReducer(initialWizardState, { type: 'imported', log });

describe('wizardReducer', () => {
  it('pre-selects rows highlighted in Excel', () => {
    expect([...imported().selectedRowIds]).toEqual([4, 6]);
  });

  it('lets the operator override the pre-selection', () => {
    let state = wizardReducer(imported(), { type: 'rowToggled', rowId: 4 });
    state = wizardReducer(state, { type: 'rowToggled', rowId: 5 });
    expect([...state.selectedRowIds].sort()).toEqual([5, 6]);

    state = wizardReducer(state, { type: 'allRowsSet', selected: false });
    expect(state.selectedRowIds.size).toBe(0);
    state = wizardReducer(state, { type: 'allRowsSet', selected: true });
    expect(state.selectedRowIds.size).toBe(4);
  });

  it('only continues when at least one row is selected', () => {
    expect(canContinueFromRows(initialWizardState)).toBe(false);
    const none = wizardReducer(imported(), { type: 'allRowsSet', selected: false });
    expect(wizardReducer(none, { type: 'stepRequested', step: 'details' }).step).toBe('rows');
    expect(wizardReducer(imported(), { type: 'stepRequested', step: 'details' }).step).toBe('details');
  });

  it('builds the draft in workbook order', () => {
    let state = wizardReducer(imported(), { type: 'rowToggled', rowId: 3 });
    state = wizardReducer(state, { type: 'systemChanged', systemId: 'system-1' });
    state = wizardReducer(state, { type: 'urlChanged', url: 'https://checks.example.com/1' });
    expect(toDraft(state)).toEqual({
      importId: 7,
      selectedRowIds: [3, 4, 6],
      systemId: 'system-1',
      preliminaryCheckUrl: 'https://checks.example.com/1',
    });
    expect(toDraft(initialWizardState)).toBeNull();
  });

  it('clears a field error when that field changes', () => {
    let state = wizardReducer(imported(), { type: 'stepRequested', step: 'details' });
    state = wizardReducer(state, {
      type: 'validationFailed',
      errors: [
        { field: 'preliminaryCheckUrl', code: 'invalid_url' },
        { field: 'systemId', code: 'required' },
      ],
    });
    state = wizardReducer(state, { type: 'urlChanged', url: 'https://x.example.com' });
    expect(state.fieldErrors).toEqual([{ field: 'systemId', code: 'required' }]);
  });

  it('returns to row selection when the backend rejects the rows', () => {
    let state = wizardReducer(imported(), { type: 'stepRequested', step: 'details' });
    state = wizardReducer(state, { type: 'validationFailed', errors: [{ field: 'rows', code: 'import_expired' }] });
    expect(state.step).toBe('rows');
  });

  it('invalidates the preview when the selection changes', () => {
    const preview = {
      number: '056-2026',
      date: '2026-09-23',
      system: { id: 'system-1', name: 'מערכת אלפא' },
      template: { name: 't', title: 't', sections: [] },
      preliminaryCheckUrl: 'https://checks.example.com/1',
      rows: [],
      sourceFileName: 'log.xlsx',
    };
    let state = wizardReducer(imported(), { type: 'previewed', preview });
    expect(state.step).toBe('preview');
    state = wizardReducer(state, { type: 'rowToggled', rowId: 3 });
    expect(state.preview).toBeNull();
  });
});
