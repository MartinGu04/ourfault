// Light / dark theme. A per-user display preference kept in local storage,
// never in investigation data. `public/theme-boot.js` applies the same
// preference before the first paint so the window never flashes.

import { useCallback, useEffect, useState } from 'react';

export type ThemePreference = 'light' | 'dark' | 'system';
export type Theme = 'light' | 'dark';

export const THEME_STORAGE_KEY = 'ourfault.theme';
export const THEME_PREFERENCES: readonly ThemePreference[] = ['light', 'dark', 'system'];

type Reader = Pick<Storage, 'getItem'>;
type Writer = Pick<Storage, 'setItem'>;

const isPreference = (value: unknown): value is ThemePreference =>
  typeof value === 'string' && (THEME_PREFERENCES as readonly string[]).includes(value);

/** The saved preference; "system" when nothing (valid) was saved or storage is unavailable. */
export function readPreference(storage: Reader | null): ThemePreference {
  try {
    const saved = storage?.getItem(THEME_STORAGE_KEY);
    return isPreference(saved) ? saved : 'system';
  } catch {
    return 'system';
  }
}

export function savePreference(storage: Writer | null, preference: ThemePreference): void {
  try {
    storage?.setItem(THEME_STORAGE_KEY, preference);
  } catch {
    // A preference that cannot be saved still applies for this session.
  }
}

export function resolveTheme(preference: ThemePreference, systemPrefersDark: boolean): Theme {
  if (preference === 'system') return systemPrefersDark ? 'dark' : 'light';
  return preference;
}

/** Sets the theme on the root element; CSS tokens follow `data-theme`. */
export function applyTheme(root: Pick<HTMLElement, 'setAttribute'>, theme: Theme): void {
  root.setAttribute('data-theme', theme);
}

const DARK_QUERY = '(prefers-color-scheme: dark)';

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function systemPrefersDark(): boolean {
  return typeof window.matchMedia === 'function' && window.matchMedia(DARK_QUERY).matches;
}

/** The current theme, applied to the document, following the OS when "system". */
export function useTheme() {
  const [preference, setPreferenceState] = useState<ThemePreference>(() => readPreference(storage()));
  const [systemDark, setSystemDark] = useState(systemPrefersDark);
  const theme = resolveTheme(preference, systemDark);

  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const query = window.matchMedia(DARK_QUERY);
    const onChange = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    if (root.getAttribute('data-theme') === theme) return;
    // Cross-fade colours briefly while switching (disabled for reduced motion in CSS).
    root.classList.add('theme-switching');
    applyTheme(root, theme);
    const timer = window.setTimeout(() => root.classList.remove('theme-switching'), 320);
    return () => window.clearTimeout(timer);
  }, [theme]);

  const setPreference = useCallback((next: ThemePreference) => {
    savePreference(storage(), next);
    setPreferenceState(next);
  }, []);

  return { preference, theme, setPreference };
}
