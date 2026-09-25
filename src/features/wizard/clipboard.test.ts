import { describe, expect, it } from 'vitest';

import { MAX_PASTE_BYTES, readClipboardText, type ClipboardLike } from './clipboard';

function clipboard(flavours: Record<string, string>, files = 0): ClipboardLike {
  return { types: Object.keys(flavours), files: { length: files }, getData: (format) => flavours[format] ?? '' };
}

describe('clipboard intake', () => {
  it('reads Excel rows from the plain-text flavour', () => {
    const excel = '08:14\tעמדה 3\tמוקד\tזוהתה האטה\r\n';
    expect(readClipboardText(clipboard({ 'text/plain': excel, 'text/html': '<table>…</table>' }))).toEqual({
      kind: 'text',
      text: excel,
    });
  });

  it('never falls back to HTML', () => {
    expect(readClipboardText(clipboard({ 'text/html': '<script>alert(1)</script>' }))).toEqual({
      kind: 'rejected',
      reason: 'not_text',
    });
  });

  it('rejects images, files and an empty clipboard', () => {
    expect(readClipboardText(clipboard({ Files: '' }, 1))).toEqual({ kind: 'rejected', reason: 'not_text' });
    expect(readClipboardText(clipboard({ 'image/png': '' }))).toEqual({ kind: 'rejected', reason: 'not_text' });
    expect(readClipboardText(clipboard({}))).toEqual({ kind: 'rejected', reason: 'empty' });
    expect(readClipboardText(clipboard({ 'text/plain': ' \r\n\t' }))).toEqual({ kind: 'rejected', reason: 'empty' });
    expect(readClipboardText(null)).toEqual({ kind: 'rejected', reason: 'empty' });
  });

  it('rejects oversized pastes before sending them', () => {
    const huge = 'א'.repeat(MAX_PASTE_BYTES / 2 + 1); // two bytes per Hebrew letter
    expect(readClipboardText(clipboard({ 'text/plain': huge }))).toEqual({ kind: 'rejected', reason: 'too_large' });
  });

  it('keeps script-like text as text for the tabular parser', () => {
    const text = "08:00\t<script>alert('x')</script>\tb\tc";
    expect(readClipboardText(clipboard({ 'text/plain': text }))).toEqual({ kind: 'text', text });
  });
});
