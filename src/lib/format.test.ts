import { describe, expect, it } from 'vitest';

import { formatDate, formatTimestamp, rowsLabel } from './format';

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
