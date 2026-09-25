// Clipboard intake for the chronology paste. Only the plain-text flavour of
// the clipboard is ever read: never HTML, images or files. The text is sent
// to the backend parser, which treats it strictly as tabular data.

/** Same limit as the backend (`MAX_PASTE_BYTES`), checked here to avoid sending huge payloads. */
export const MAX_PASTE_BYTES = 2 * 1024 * 1024;

export const NOT_TABULAR_MESSAGE = 'לא זוהה מידע טבלאי מיומן המבצעים. יש להעתיק שורות מ-Excel ולהדביק אותן כאן.';

/** The parts of `DataTransfer` used here (so tests need no browser). */
export interface ClipboardLike {
  readonly types: readonly string[];
  readonly files?: { readonly length: number };
  getData(format: string): string;
}

export type ClipboardRead =
  | { kind: 'text'; text: string }
  | { kind: 'rejected'; reason: 'empty' | 'not_text' | 'too_large' };

export function readClipboardText(data: ClipboardLike | null | undefined): ClipboardRead {
  if (!data) return { kind: 'rejected', reason: 'empty' };
  const hasText = Array.from(data.types).includes('text/plain');
  const text = hasText ? data.getData('text/plain') : '';
  if (text.trim() === '') {
    // An image, a file or other non-text content, or nothing at all.
    const other = (data.files?.length ?? 0) > 0 || data.types.length > 0;
    return { kind: 'rejected', reason: other && !hasText ? 'not_text' : 'empty' };
  }
  if (new TextEncoder().encode(text).length > MAX_PASTE_BYTES) {
    return { kind: 'rejected', reason: 'too_large' };
  }
  return { kind: 'text', text };
}
