import { describe, expect, it } from 'vitest';

import { advisoryMessage, fieldMessage } from '../../api/errors';
import { splitIssues } from './ReviewStep';

describe('review summary', () => {
  it('separates missing required values from invalid values and ranges', () => {
    const { missing, invalid } = splitIssues([
      { field: 'plannedStart', code: 'required' },
      { field: 'actualEnd', code: 'end_before_start' },
      { field: 'rows', code: 'no_rows' },
      { field: 'plannedEnd', code: 'invalid_datetime' },
    ]);
    expect(missing.map((i) => i.field)).toEqual(['plannedStart', 'rows']);
    expect(invalid.map((i) => i.field)).toEqual(['actualEnd', 'plannedEnd']);
  });

  it('names which range is wrong', () => {
    expect(fieldMessage({ field: 'plannedEnd', code: 'end_before_start' })).toBe('סיום התכנון מוקדם מההתחלה המתוכננת');
    expect(fieldMessage({ field: 'actualEnd', code: 'end_before_start' })).toBe('הסיום בפועל מוקדם מההתחלה בפועל');
  });
});

describe('advisory wording', () => {
  it('words warnings and information by code and errors by field', () => {
    expect(advisoryMessage({ severity: 'warning', field: 'nightActivity', code: 'night_overlap_not_marked' })).toBe(
      'הפעילות כוללת זמן בטווח שעות הלילה שהוגדר, אך סומנה כמשימת לילה: לא. מומלץ לבדוק.',
    );
    expect(advisoryMessage({ severity: 'info', field: 'actualStart', code: 'actual_started_before_plan' })).toBe(
      'הביצוע בפועל החל לפני מועד ההתחלה המתוכנן.',
    );
    expect(advisoryMessage({ severity: 'error', field: 'actualEnd', code: 'end_before_start' })).toBe(
      'הסיום בפועל מוקדם מההתחלה בפועל',
    );
  });
});
