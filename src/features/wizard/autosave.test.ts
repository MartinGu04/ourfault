import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { Autosaver, type SaveStatus } from './autosave';

function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('Autosaver', () => {
  let writes: { value: string; done: ReturnType<typeof deferred> }[];
  let statuses: SaveStatus[];

  function autosaver(isFatal?: (error: unknown) => boolean) {
    return new Autosaver<string>({
      delayMs: 800,
      persist: (value) => {
        const done = deferred();
        writes.push({ value, done });
        return done.promise;
      },
      onStatus: (status) => statuses.push(status),
      isFatal,
      timers: { set: (fn, ms) => setTimeout(fn, ms), clear: (handle) => clearTimeout(handle as number) },
    });
  }

  beforeEach(() => {
    vi.useFakeTimers();
    writes = [];
    statuses = [];
  });
  afterEach(() => vi.useRealTimers());

  it('debounces changes into one write of the latest value', async () => {
    const saver = autosaver();
    saver.update('a');
    await vi.advanceTimersByTimeAsync(500);
    saver.update('ab');
    await vi.advanceTimersByTimeAsync(500);
    saver.update('abc');
    expect(writes).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(800);
    expect(writes.map((w) => w.value)).toEqual(['abc']);
    writes[0]!.done.resolve();
    await vi.runAllTimersAsync();
    expect(statuses.at(-1)).toBe('saved');
  });

  it('keeps one write in flight and saves later changes right after it', async () => {
    const saver = autosaver();
    saver.update('1');
    await vi.advanceTimersByTimeAsync(800);
    saver.update('2');
    await vi.advanceTimersByTimeAsync(800);
    expect(writes.map((w) => w.value)).toEqual(['1']);
    writes[0]!.done.resolve();
    await vi.advanceTimersByTimeAsync(0);
    expect(writes.map((w) => w.value)).toEqual(['1', '2']);
  });

  it('keeps unsaved work after a failure and retries on flush', async () => {
    const saver = autosaver();
    saver.update('work');
    await vi.advanceTimersByTimeAsync(800);
    writes[0]!.done.reject(new Error('offline'));
    await vi.advanceTimersByTimeAsync(0);
    expect(statuses.at(-1)).toBe('error');
    expect(saver.hasUnsavedChanges).toBe(true);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(writes).toHaveLength(1);

    const flushed = saver.flush();
    await vi.advanceTimersByTimeAsync(0);
    expect(writes.map((w) => w.value)).toEqual(['work', 'work']);
    writes[1]!.done.resolve();
    expect(await flushed).toBe(true);
    expect(saver.hasUnsavedChanges).toBe(false);
  });

  it('a failing flush gives up after one attempt', async () => {
    const saver = autosaver();
    saver.update('x');
    const flushed = saver.flush();
    await vi.advanceTimersByTimeAsync(0);
    writes[0]!.done.reject(new Error('offline'));
    expect(await flushed).toBe(false);
    expect(writes).toHaveLength(1);
  });

  it('stops after a fatal error so nothing is overwritten', async () => {
    const saver = autosaver(() => true);
    saver.update('mine');
    await vi.advanceTimersByTimeAsync(800);
    writes[0]!.done.reject(new Error('conflict'));
    await vi.advanceTimersByTimeAsync(0);
    saver.update('more');
    await vi.advanceTimersByTimeAsync(5_000);
    expect(writes).toHaveLength(1);
    expect(await saver.flush()).toBe(false);
  });
});
