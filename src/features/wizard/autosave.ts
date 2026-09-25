// Debounced autosave. Changes are collected and written after a short quiet
// period; at most one write is in flight, and changes made during a write are
// saved right after it. A failed write keeps the latest value in memory (and
// retries with the next change or an explicit flush); a fatal failure, such
// as a conflict with another workstation, stops autosaving so nothing is
// overwritten.

export type SaveStatus = 'idle' | 'pending' | 'saving' | 'saved' | 'error';

export interface AutosaveOptions<T> {
  persist: (value: T) => Promise<void>;
  onStatus: (status: SaveStatus, error?: unknown) => void;
  /** Errors after which autosave stops for good (e.g. a conflict). */
  isFatal?: ((error: unknown) => boolean) | undefined;
  delayMs?: number;
  timers?: { set: (fn: () => void, ms: number) => unknown; clear: (handle: unknown) => void };
}

export class Autosaver<T> {
  private pending: { value: T } | null = null;
  private inFlight: Promise<void> | null = null;
  private timer: unknown = null;
  private halted = false;
  private readonly delayMs: number;
  private readonly timers: NonNullable<AutosaveOptions<T>['timers']>;

  constructor(private readonly options: AutosaveOptions<T>) {
    this.delayMs = options.delayMs ?? 800;
    this.timers = options.timers ?? {
      set: (fn, ms) => window.setTimeout(fn, ms),
      clear: (handle) => window.clearTimeout(handle as number),
    };
  }

  /** Records a change; it is written after the quiet period. */
  update(value: T): void {
    this.pending = { value };
    if (this.halted) return;
    this.options.onStatus('pending');
    this.clearTimer();
    this.timer = this.timers.set(() => {
      this.timer = null;
      this.run();
    }, this.delayMs);
  }

  /**
   * Writes any pending change now (one attempt) and waits for it. Resolves
   * to true when everything is saved.
   */
  async flush(): Promise<boolean> {
    this.clearTimer();
    if (this.inFlight) await this.inFlight;
    if (this.pending) this.run();
    while (this.inFlight) await this.inFlight;
    return !this.pending && !this.halted;
  }

  get hasUnsavedChanges(): boolean {
    return this.pending !== null || this.inFlight !== null;
  }

  dispose(): void {
    this.clearTimer();
    this.halted = true;
  }

  private clearTimer() {
    if (this.timer !== null) {
      this.timers.clear(this.timer);
      this.timer = null;
    }
  }

  /** Starts a write of the pending value, if possible. */
  private run(): void {
    if (this.inFlight || this.halted || !this.pending) return;
    const { value } = this.pending;
    this.pending = null;
    this.options.onStatus('saving');
    let failed = false;
    this.inFlight = this.options
      .persist(value)
      .then(
        () => {
          if (!this.pending) this.options.onStatus('saved');
        },
        (error: unknown) => {
          failed = true;
          // Keep the unsaved value unless a newer one is already waiting.
          this.pending ??= { value };
          if (this.options.isFatal?.(error)) this.halted = true;
          this.options.onStatus('error', error);
        },
      )
      .finally(() => {
        this.inFlight = null;
        // Changes made during the write are saved right away; after a
        // failure, only the next change (or a flush) tries again.
        if (!failed && this.pending && this.timer === null) this.run();
      });
  }
}
