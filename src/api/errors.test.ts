import { describe, expect, it } from 'vitest';

import { ApiError, errorMessage, fieldErrors, toAppError } from './errors';

describe('toAppError', () => {
  it('accepts well-formed backend errors', () => {
    expect(toAppError({ kind: 'validation', errors: [{ field: 'systemIds', code: 'required' }] })).toEqual({
      kind: 'validation',
      errors: [{ field: 'systemIds', code: 'required' }],
    });
    expect(toAppError({ kind: 'conflict', code: 'changed' })).toEqual({ kind: 'conflict', code: 'changed' });
    expect(toAppError({ kind: 'unavailable' })).toEqual({ kind: 'unavailable' });
    expect(toAppError({ kind: 'paste', code: 'too_few_columns' })).toEqual({ kind: 'paste', code: 'too_few_columns' });
  });

  it('treats anything unexpected as an internal error', () => {
    expect(toAppError('Error: C:\\Users\\x\\file.json not found')).toEqual({ kind: 'internal' });
    expect(toAppError({ kind: 'validation', errors: 'oops' })).toEqual({ kind: 'internal' });
    expect(toAppError(null)).toEqual({ kind: 'internal' });
  });
});

describe('messages', () => {
  it('maps field errors to Hebrew messages, with field-specific wording', () => {
    const error = new ApiError({
      kind: 'validation',
      errors: [
        { field: 'systemIds', code: 'required' },
        { field: 'preliminaryCheckUrl', code: 'unsupported_scheme' },
      ],
    });
    expect(fieldErrors(error)).toEqual({
      systemIds: 'יש לבחור לפחות מערכת אחת',
      preliminaryCheckUrl: 'הכתובת צריכה להתחיל ב-\u2066https://\u2069 או ב-\u2066http://\u2069',
    });
  });

  it('explains an unreachable destination without claiming success', () => {
    const message = errorMessage(new ApiError({ kind: 'unavailable' }));
    expect(message).toContain('לא נוצר');
    expect(message).toContain('הטיוטה שמורה');
  });

  it('never exposes raw error text', () => {
    expect(errorMessage(new Error('stack trace at C:\\app'))).not.toContain('C:\\');
    expect(errorMessage(new ApiError({ kind: 'paste', code: 'too_few_columns' }))).toContain('ארבע עמודות');
  });
});
