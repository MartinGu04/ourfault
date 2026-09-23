import { describe, expect, it } from 'vitest';

import type { LogRow, PastedRows } from '../../api/types';
import { canContinueFromRows, initialWizardState, toDraft, wizardReducer, type WizardState } from './wizardState';

function row(time: string, description: string): LogRow {
  return { time, from: 'א', to: 'ב', description };
}

const paste = (rows: LogRow[], headerSkipped = false): PastedRows => ({ rows, headerSkipped });

const pasted = (): WizardState =>
  wizardReducer(initialWizardState, { type: 'rowsPasted', pasted: paste([row('08:00', 'a'), row('08:10', 'b')]) });

describe('wizardReducer', () => {
  it('adds pasted rows for review, appending on further pastes', () => {
    let state = pasted();
    expect(state.rows.map((r) => r.description)).toEqual(['a', 'b']);
    state = wizardReducer(state, { type: 'rowsPasted', pasted: paste([row('09:00', 'c')], true) });
    expect(state.rows.map((r) => r.description)).toEqual(['a', 'b', 'c']);
    expect(new Set(state.rows.map((r) => r.key)).size).toBe(3);
    expect(state.lastPaste).toEqual({ count: 1, headerSkipped: true });
  });

  it('lets the operator edit and remove rows', () => {
    let state = pasted();
    const [first, second] = state.rows;
    state = wizardReducer(state, { type: 'rowUpdated', key: first!.key, row: row('08:01', 'a (תוקן)') });
    state = wizardReducer(state, { type: 'rowRemoved', key: second!.key });
    expect(state.rows).toEqual([{ ...row('08:01', 'a (תוקן)'), key: first!.key }]);

    state = wizardReducer(state, { type: 'rowsCleared' });
    expect(state.rows).toEqual([]);
  });

  it('only continues when there is at least one row', () => {
    expect(canContinueFromRows(initialWizardState)).toBe(false);
    expect(wizardReducer(initialWizardState, { type: 'stepRequested', step: 'details' }).step).toBe('rows');
    expect(wizardReducer(pasted(), { type: 'stepRequested', step: 'details' }).step).toBe('details');
  });

  it('sends exactly the reviewed rows in their order', () => {
    let state = wizardReducer(pasted(), { type: 'systemChanged', systemId: 'system-1' });
    state = wizardReducer(state, { type: 'urlChanged', url: 'https://checks.example.com/1' });
    expect(toDraft(state)).toEqual({
      systemId: 'system-1',
      preliminaryCheckUrl: 'https://checks.example.com/1',
      rows: [row('08:00', 'a'), row('08:10', 'b')],
    });
  });

  it('clears a field error when that field changes', () => {
    let state = wizardReducer(pasted(), { type: 'stepRequested', step: 'details' });
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

  it('returns to the rows step when the backend rejects the rows', () => {
    let state = wizardReducer(pasted(), { type: 'stepRequested', step: 'details' });
    state = wizardReducer(state, { type: 'validationFailed', errors: [{ field: 'rows', code: 'empty_row' }] });
    expect(state.step).toBe('rows');
  });

  it('invalidates the preview when the rows change', () => {
    const preview = {
      number: '056-2026',
      date: '2026-09-23',
      system: { id: 'system-1', name: 'מערכת אלפא' },
      template: { name: 't', title: 't', sections: [] },
      preliminaryCheckUrl: 'https://checks.example.com/1',
      rows: [],
    };
    let state = wizardReducer(pasted(), { type: 'previewed', preview });
    expect(state.step).toBe('preview');
    state = wizardReducer(state, { type: 'rowRemoved', key: state.rows[0]!.key });
    expect(state.preview).toBeNull();
  });
});
