import { describe, expect, it } from 'vitest';

import { formatDate, formatTimestamp, rowsLabel } from './format';
import { describeField } from './labels';
import type { SectionDefinition } from '../api/types';

describe('format', () => {
  it('formats dates without time-zone conversion', () => {
    expect(formatDate('2026-09-23')).toBe('23/09/2026');
    expect(formatTimestamp('2026-09-23T23:59:00-05:00')).toBe('23/09/2026 23:59');
    expect(formatDate('not a date')).toBe('not a date');
  });

  it('uses Hebrew singular and plural forms', () => {
    expect(rowsLabel(1)).toBe('שורה אחת');
    expect(rowsLabel(6)).toBe('6 שורות');
  });
});

describe('describeField', () => {
  const sections: SectionDefinition[] = [
    {
      id: 'section-2',
      name: 'פרטי כלים',
      active: true,
      mode: 'repeating',
      fields: [{ id: 'f3', label: 'מס׳ זנב', kind: 'text', required: true, active: true }],
    },
  ];

  it('names configured section fields with their row', () => {
    expect(describeField('sections.section-2.1.f3', sections)).toEqual({
      label: 'פרטי כלים · שורה 2 · מס׳ זנב',
      step: 'technical',
    });
  });

  it('points top-level fields at their step', () => {
    expect(describeField('rows', sections)).toEqual({ label: 'השתלשלות אירועים', step: 'chronology' });
    expect(describeField('actualEnd', sections).step).toBe('activity');
  });
});
