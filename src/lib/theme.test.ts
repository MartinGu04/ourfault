import { describe, expect, it } from 'vitest';

import bootScript from '../../public/theme-boot.js?raw';
import { applyTheme, readPreference, resolveTheme, savePreference, THEME_STORAGE_KEY } from './theme';

function memoryStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
    data,
  };
}

const throwing = {
  getItem: () => {
    throw new Error('blocked');
  },
  setItem: () => {
    throw new Error('blocked');
  },
};

describe('theme preference', () => {
  it('is saved locally and read back on the next launch', () => {
    const storage = memoryStorage();
    expect(readPreference(storage)).toBe('system');
    savePreference(storage, 'dark');
    expect(storage.data.get(THEME_STORAGE_KEY)).toBe('dark');
    expect(readPreference(storage)).toBe('dark');
  });

  it('falls back to the system theme for unknown values or unavailable storage', () => {
    expect(readPreference(memoryStorage({ [THEME_STORAGE_KEY]: 'sepia' }))).toBe('system');
    expect(readPreference(throwing)).toBe('system');
    expect(readPreference(null)).toBe('system');
    expect(() => savePreference(throwing, 'light')).not.toThrow();
  });

  it('resolves "system" from the operating system and keeps explicit choices', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });

  it('is applied to the root element as data-theme', () => {
    const attributes = new Map<string, string>();
    applyTheme({ setAttribute: (name, value) => void attributes.set(name, value) }, 'dark');
    expect(attributes.get('data-theme')).toBe('dark');
  });
});

describe('theme boot script (runs before the first paint)', () => {
  function boot(saved: string | null, systemDark: boolean): string | undefined {
    const attributes = new Map<string, string>();
    const window = {
      localStorage: memoryStorage(saved === null ? {} : { [THEME_STORAGE_KEY]: saved }),
      matchMedia: () => ({ matches: systemDark }),
    };
    const document = { documentElement: { setAttribute: (name: string, value: string) => attributes.set(name, value) } };
    new Function('window', 'document', bootScript)(window, document);
    return attributes.get('data-theme');
  }

  it('matches the app: saved choice first, otherwise the system theme', () => {
    expect(boot('dark', false)).toBe('dark');
    expect(boot('light', true)).toBe('light');
    expect(boot('system', true)).toBe('dark');
    expect(boot(null, false)).toBe('light');
    expect(boot('bogus', true)).toBe('dark');
  });
});
