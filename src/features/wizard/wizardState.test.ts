import { describe, expect, it } from 'vitest';

import type { Draft, LogRow, PastedRows } from '../../api/types';
import { initialWizardState, stateFromDraft, toContent, wizardReducer, type WizardState } from './wizardState';

function row(time: string, description: string): LogRow {
  return { time, from: 'א', to: 'ב', description };
}

const paste = (rows: LogRow[], headerSkipped = false): PastedRows => ({ rows, headerSkipped });

const pasted = (): WizardState =>
  wizardReducer(initialWizardState, { type: 'rowsPasted', pasted: paste([row('08:00', 'a'), row('08:10', 'b')]) });

describe('chronology rows (paste and review, unchanged from PR #1)', () => {
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

  it('saves exactly the reviewed rows in their order', () => {
    expect(toContent(pasted()).rows).toEqual([row('08:00', 'a'), row('08:10', 'b')]);
  });
});

describe('activity and technical sections', () => {
  it('selects several systems and clears their error', () => {
    let state = wizardReducer(initialWizardState, {
      type: 'validationFailed',
      errors: [
        { field: 'systemIds', code: 'required' },
        { field: 'activityName', code: 'required' },
      ],
    });
    state = wizardReducer(state, { type: 'activityChanged', patch: { systemIds: ['system-1', 'system-3'] } });
    expect(toContent(state).activity.systemIds).toEqual(['system-1', 'system-3']);
    expect(state.fieldErrors).toEqual([{ field: 'activityName', code: 'required' }]);
  });

  it('keeps partially filled times as typed (drafts are never blocked)', () => {
    const state = wizardReducer(initialWizardState, {
      type: 'activityChanged',
      patch: { plannedStart: '2026-09-20T08:00', actualEnd: '2026-09-19T08:00' },
    });
    expect(toContent(state).activity).toMatchObject({
      plannedStart: '2026-09-20T08:00',
      plannedEnd: '',
      actualStart: '',
      actualEnd: '2026-09-19T08:00',
    });
  });

  it('changing the activity status revalidates the actual end', () => {
    let state = wizardReducer(initialWizardState, {
      type: 'validationFailed',
      errors: [{ field: 'actualEnd', code: 'required' }],
    });
    state = wizardReducer(state, { type: 'activityChanged', patch: { status: 'active' } });
    expect(state.fieldErrors).toEqual([]);
  });

  it('keeps repeated technical rows and their values', () => {
    let state = wizardReducer(initialWizardState, { type: 'sectionRowAdded', sectionId: 'section-2' });
    state = wizardReducer(state, { type: 'sectionRowAdded', sectionId: 'section-2' });
    state = wizardReducer(state, { type: 'sectionValueChanged', sectionId: 'section-2', index: 1, fieldId: 'f3', value: '4X-1' });
    state = wizardReducer(state, { type: 'sectionValueChanged', sectionId: 'section-2', index: 0, fieldId: 'f3', value: '4X-0' });
    expect(toContent(state).sections['section-2']).toEqual([{ f3: '4X-0' }, { f3: '4X-1' }]);

    state = wizardReducer(state, { type: 'sectionRowRemoved', sectionId: 'section-2', index: 0 });
    expect(toContent(state).sections['section-2']).toEqual([{ f3: '4X-1' }]);
  });

  it('creates the record of a single-record section on first input and clears values', () => {
    let state = wizardReducer(initialWizardState, {
      type: 'sectionValueChanged',
      sectionId: 'section-3',
      index: 0,
      fieldId: 'f2',
      value: false,
    });
    expect(toContent(state).sections['section-3']).toEqual([{ f2: false }]);
    state = wizardReducer(state, { type: 'sectionValueChanged', sectionId: 'section-3', index: 0, fieldId: 'f2', value: null });
    expect(toContent(state).sections['section-3']).toEqual([{}]);
  });

  it('removing a row drops the (now misplaced) errors of that section', () => {
    let state = wizardReducer(initialWizardState, { type: 'sectionRowAdded', sectionId: 's' });
    state = wizardReducer(state, {
      type: 'validationFailed',
      errors: [
        { field: 'sections.s.0.f1', code: 'required' },
        { field: 'rows', code: 'no_rows' },
      ],
    });
    state = wizardReducer(state, { type: 'sectionRowRemoved', sectionId: 's', index: 0 });
    expect(state.fieldErrors).toEqual([{ field: 'rows', code: 'no_rows' }]);
  });
});

describe('drafts', () => {
  const draft: Draft = {
    id: 'd-20260923-00000000000000a1',
    revision: 4,
    createdAt: '2026-09-23T08:45:00+03:00',
    createdBy: 'op',
    updatedAt: '2026-09-23T09:10:00+03:00',
    step: 'technical',
    content: {
      activity: { ...initialWizardState.activity, name: 'בלט רומני', systemIds: ['system-1'] },
      sections: { 'section-1': [{ f1: 'x' }] },
      rows: [row('08:00', 'a')],
      preliminaryCheckUrl: 'https://checks.example.com/1',
    },
  };

  it('continues a saved draft where the operator stopped', () => {
    const state = stateFromDraft(draft);
    expect(state.step).toBe('technical');
    expect(toContent(state)).toEqual(draft.content);
    const next = wizardReducer(state, { type: 'rowsPasted', pasted: paste([row('09:00', 'b')]) });
    expect(new Set(next.rows.map((r) => r.key)).size).toBe(2);
  });

  it('keeps the number shown during review to explain a change after creation', () => {
    let state = wizardReducer(initialWizardState, { type: 'reviewed', issues: [], expectedNumber: '056-2026' });
    state = wizardReducer(state, { type: 'validationFailed', errors: [] });
    expect(state.expectedNumber).toBe('056-2026');
  });
});

describe('step transition direction', () => {
  it('records whether the operator moved forward or back', () => {
    let state = wizardReducer(initialWizardState, { type: 'stepRequested', step: 'technical' });
    expect(state.direction).toBe('forward');
    state = wizardReducer(state, { type: 'stepRequested', step: 'review' });
    expect(state.direction).toBe('forward');
    state = wizardReducer(state, { type: 'stepRequested', step: 'activity' });
    expect(state.direction).toBe('back');
    // Other changes keep the last direction (no transition is replayed for them).
    state = wizardReducer(state, { type: 'activityChanged', patch: { name: 'x' } });
    expect(state.direction).toBe('back');
  });
});
