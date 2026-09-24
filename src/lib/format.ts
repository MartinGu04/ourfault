// Display formatting. Dates are formatted from their string parts so that the
// value shown is exactly what the backend recorded, with no time-zone shifts.

const ISO_DATE = /^(\d{4})-(\d{2})-(\d{2})/;
const ISO_TIME = /T(\d{2}):(\d{2})/;

/** "2026-09-23" → "23/09/2026". Unrecognised input is returned unchanged. */
export function formatDate(iso: string): string {
  const match = ISO_DATE.exec(iso);
  if (!match) return iso;
  const [, year, month, day] = match;
  return `${day}/${month}/${year}`;
}

/** "2026-09-23T10:05:00+03:00" → "23/09/2026 10:05". */
export function formatTimestamp(iso: string): string {
  const time = ISO_TIME.exec(iso);
  return time ? `${formatDate(iso)} ${time[1]}:${time[2]}` : formatDate(iso);
}

/** "10:05" from an RFC 3339 timestamp. */
export function formatTime(iso: string): string {
  const time = ISO_TIME.exec(iso);
  return time ? `${time[1]}:${time[2]}` : '';
}

/** Hebrew count phrase: "שורה אחת", "2 שורות". */
export function countLabel(count: number, one: string, many: string): string {
  return count === 1 ? one : `${count.toLocaleString('he-IL')} ${many}`;
}

export const rowsLabel = (count: number) => countLabel(count, 'שורה אחת', 'שורות');
export const recipientsLabel = (count: number) => countLabel(count, 'נמען אחד', 'נמענים');
