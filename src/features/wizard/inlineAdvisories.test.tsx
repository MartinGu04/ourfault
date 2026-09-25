// Findings from the backend shown next to their fields while editing, and
// still listed together on the review step. The rules themselves are the
// backend's (Rust tests); these tests use the codes it produces.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { Advisory, LogRow, Review, Workspace } from '../../api/types';
import { ActivityStep } from './ActivityStep';
import { PasteRowsStep } from './PasteRowsStep';
import { ReviewSummary } from './ReviewStep';
import { TechnicalStep } from './TechnicalStep';
import {
  blockingIssues,
  initialWizardState,
  pasteFeedback,
  visibleErrors,
  wizardReducer,
  type WizardAction,
  type WizardState,
} from './wizardState';

const NIGHT_WARNING = 'הפעילות כוללת זמן בטווח שעות הלילה';
const STARTED_EARLY = 'הביצוע בפועל החל לפני מועד ההתחלה המתוכנן.';
const ENDED_LATE = 'הביצוע בפועל הסתיים לאחר מועד הסיום המתוכנן.';
const PLANNED_REVERSED = 'סיום התכנון מוקדם מההתחלה המתוכננת';
const ACTUAL_REVERSED = 'הסיום בפועל מוקדם מההתחלה בפועל';
const NAME_REQUIRED_FIELD = 'activityName';

const warning: Advisory = { severity: 'warning', code: 'night_overlap_not_marked', field: 'nightActivity' };
const startedEarly: Advisory = { severity: 'info', code: 'actual_started_before_plan', field: 'actualStart' };
const endedLate: Advisory = { severity: 'info', code: 'actual_ended_after_plan', field: 'actualEnd' };
const plannedReversed: Advisory = { severity: 'error', code: 'end_before_start', field: 'plannedEnd' };
const actualReversed: Advisory = { severity: 'error', code: 'end_before_start', field: 'actualEnd' };
const nameMissing: Advisory = { severity: 'error', code: 'required', field: NAME_REQUIRED_FIELD };

const workspace: Workspace = {
  systems: [{ id: 'system-1', name: 'מערכת אלפא', active: true }],
  stations: [{ id: 'station-1', name: 'תחנה א', active: true }],
  sections: [
    {
      id: 'section-2',
      name: 'פרטי כלים',
      active: true,
      mode: 'repeating',
      fields: [{ id: 'f1', label: 'עמודה 1', kind: 'text', required: true, active: true }],
    },
  ],
  setup: { items: [], ready: true },
};

const noop = () => undefined;

const run = (state: WizardState, ...actions: WizardAction[]) => actions.reduce(wizardReducer, state);
const assessed = (...advisories: Advisory[]): WizardState =>
  run(initialWizardState, { type: 'assessed', advisories });

const activityHtml = (state: WizardState) =>
  renderToStaticMarkup(<ActivityStep state={state} dispatch={noop} workspace={workspace} />);

/** The markup of one card, found by its heading. */
function card(html: string, heading: string): string {
  const start = html.lastIndexOf('<section', html.indexOf(`>${heading}</h2>`));
  return html.slice(start, html.indexOf('</section>', start));
}

describe('Activity details: inline findings', () => {
  it('shows the night warning next to the night-activity answer, with the quick fix', () => {
    const html = activityHtml(assessed(warning));
    const general = card(html, 'פרטי הפעילות');
    expect(general).toContain(NIGHT_WARNING);
    expect(general).toContain('סמן כמשימת לילה');
    expect(general).toContain('inline-note-warning');
  });

  it('the quick fix answers "yes" and the warning disappears at once', () => {
    let state = run(assessed(warning), { type: 'activityChanged', patch: { nightActivity: true } });
    expect(state.activity.nightActivity).toBe(true);
    expect(activityHtml(state)).not.toContain(NIGHT_WARNING);
    // The backend's re-assessment agrees: no warning comes back.
    state = run(state, { type: 'assessed', advisories: [] });
    expect(activityHtml(state)).not.toContain(NIGHT_WARNING);
  });

  it('shows planned-versus-actual information in the actual-execution card', () => {
    const html = activityHtml(assessed(startedEarly, endedLate));
    const actual = card(html, 'ביצוע בפועל');
    expect(actual).toContain(STARTED_EARLY);
    expect(actual).toContain(ENDED_LATE);
    expect(actual).toContain('inline-note-info');
    expect(card(html, 'תכנון')).not.toContain(STARTED_EARLY);
  });

  it('shows a reversed planned range in the planning card right away', () => {
    const html = activityHtml(assessed(plannedReversed));
    expect(card(html, 'תכנון')).toContain(PLANNED_REVERSED);
  });

  it('shows a reversed actual range in the actual-execution card right away', () => {
    const html = activityHtml(assessed(actualReversed));
    expect(card(html, 'ביצוע בפועל')).toContain(ACTUAL_REVERSED);
  });

  it('does not open a new draft as a wall of red: missing values wait for the operator', () => {
    const fresh = assessed(nameMissing, { severity: 'error', code: 'required', field: 'plannedStart' });
    expect(visibleErrors(fresh)).toEqual([]);
    expect(activityHtml(fresh)).not.toContain('field-invalid');

    // After leaving the field...
    const left = run(fresh, { type: 'fieldTouched', field: NAME_REQUIRED_FIELD });
    expect(visibleErrors(left).map((e) => e.field)).toEqual([NAME_REQUIRED_FIELD]);
    // ...or after moving on from the step, everything missing there shows.
    const movedOn = run(fresh, { type: 'stepRequested', step: 'technical' });
    expect(visibleErrors(movedOn).map((e) => e.field)).toEqual([NAME_REQUIRED_FIELD, 'plannedStart']);
  });

  it('a required description appears once "other" is chosen, and is flagged after leaving it', () => {
    let state = run(initialWizardState, { type: 'activityChanged', patch: { activityType: 'other' } });
    state = run(state, { type: 'assessed', advisories: [{ severity: 'error', code: 'required', field: 'activityTypeOther' }] });
    expect(visibleErrors(state)).toEqual([]);
    state = run(state, { type: 'fieldTouched', field: 'activityTypeOther' });
    expect(visibleErrors(state)).toEqual([{ field: 'activityTypeOther', code: 'required' }]);
  });
});

describe('Technical sections: inline errors', () => {
  const withRow = run(initialWizardState, { type: 'sectionRowAdded', sectionId: 'section-2' });
  const missing: Advisory = { severity: 'error', code: 'required', field: 'sections.section-2.0.f1' };
  const invalid: Advisory = { severity: 'error', code: 'unknown_station', field: 'sections.section-2.0.f1' };
  const html = (state: WizardState) =>
    renderToStaticMarkup(<TechnicalStep state={state} dispatch={noop} workspace={workspace} />);

  it('flags a missing required value after the field was left', () => {
    const state = run(withRow, { type: 'assessed', advisories: [missing] });
    expect(html(state)).not.toContain('cell-error');
    expect(html(run(state, { type: 'fieldTouched', field: missing.field }))).toContain('שדה חובה');
  });

  it('flags an invalid value right away', () => {
    expect(html(run(withRow, { type: 'assessed', advisories: [invalid] }))).toContain('התחנה שנבחרה אינה קיימת');
  });
});

describe('the review step still lists everything', () => {
  it('shows the same findings together, and only errors block creation', () => {
    const advisories = [plannedReversed, warning, startedEarly];
    const review: Review = {
      advisories,
      document: { title: 'תחקיר', subtitle: '', number: null, status: 'draft', draftNotice: null, blocks: [] },
      expectedNumber: '056-2026',
    };
    const state = run(assessed(...advisories), {
      type: 'reviewed',
      issues: blockingIssues(advisories),
      expectedNumber: '056-2026',
    });
    const html = renderToStaticMarkup(
      <ReviewSummary review={review} state={state} dispatch={noop} workspace={workspace} onGoTo={noop} />,
    );
    expect(html).toContain(PLANNED_REVERSED);
    expect(html).toContain(NIGHT_WARNING);
    expect(html).toContain(STARTED_EARLY);
    expect(html).toContain('סמן כמשימת לילה');
    expect(state.fieldErrors).toEqual([{ field: 'plannedEnd', code: 'end_before_start' }]);
  });

  it('warnings and information never block: continuing and creating stay possible', () => {
    let state = assessed(warning, startedEarly, endedLate);
    for (const step of ['technical', 'chronology', 'review'] as const) {
      state = run(state, { type: 'stepRequested', step });
      expect(state.step).toBe(step);
    }
    state = run(state, { type: 'reviewed', issues: blockingIssues([warning, startedEarly]), expectedNumber: null });
    expect(state.fieldErrors).toEqual([]); // the create button is enabled only then
  });

  it('errors still block creation', () => {
    expect(blockingIssues([warning, actualReversed, nameMissing])).toEqual([
      { field: 'actualEnd', code: 'end_before_start' },
      { field: NAME_REQUIRED_FIELD, code: 'required' },
    ]);
  });

  it('reaching the review reveals every missing value on the earlier steps', () => {
    const state = run(assessed(nameMissing), { type: 'stepRequested', step: 'review' });
    expect(visibleErrors(state)).toEqual([{ field: NAME_REQUIRED_FIELD, code: 'required' }]);
  });
});

describe('chronology paste feedback', () => {
  const row = (time: string): LogRow => ({ time, from: 'א', to: 'ב', description: 'תוכן' });

  it('says how many rows were added and how many there are now', () => {
    let state = run(initialWizardState, { type: 'rowsPasted', pasted: { rows: [row('08:00')], headerSkipped: false } });
    state = run(state, { type: 'rowsPasted', pasted: { rows: [row('09:00'), row('09:05')], headerSkipped: true } });
    const html = renderToStaticMarkup(
      <PasteRowsStep state={state} dispatch={noop} onPasteText={noop} parsing={false} />,
    );
    expect(html).toContain('נוספו 2 שורות (שורת הכותרות הושמטה). סה״כ 3 שורות בתחקיר.');
    expect(html).toContain('inline-note-success');
  });

  it('words single rows naturally', () => {
    expect(pasteFeedback({ count: 1, headerSkipped: false }, 1)).toBe('נוספה שורה אחת. סה״כ שורה אחת בתחקיר.');
    expect(pasteFeedback({ count: 7, headerSkipped: false }, 12)).toBe('נוספו 7 שורות. סה״כ 12 שורות בתחקיר.');
  });
});
