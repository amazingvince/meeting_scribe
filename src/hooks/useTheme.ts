import { useEffect } from 'react';
import { useSettingsStore } from '../stores';

const THEME_STORAGE_KEY = 'meeting-scribe-settings';
const SYSTEM_DARK_QUERY = '(prefers-color-scheme: dark)';

export type ThemePreference = 'light' | 'dark' | 'system';
type ResolvedTheme = 'light' | 'dark';

function parseThemePreference(value: unknown): ThemePreference {
  if (value === 'light' || value === 'dark' || value === 'system') {
    return value;
  }
  return 'system';
}

function getSystemTheme(): ResolvedTheme {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return 'light';
  }
  return window.matchMedia(SYSTEM_DARK_QUERY).matches ? 'dark' : 'light';
}

function resolveTheme(theme: ThemePreference): ResolvedTheme {
  return theme === 'system' ? getSystemTheme() : theme;
}

function applyResolvedTheme(theme: ResolvedTheme): void {
  if (typeof document === 'undefined') return;

  const root = document.documentElement;
  root.classList.toggle('dark', theme === 'dark');
  root.style.colorScheme = theme;
}

function readStoredThemePreference(): ThemePreference {
  if (typeof window === 'undefined') {
    return 'system';
  }

  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (!stored) return 'system';

    const parsed = JSON.parse(stored) as { state?: { theme?: unknown } };
    return parseThemePreference(parsed?.state?.theme);
  } catch {
    return 'system';
  }
}

export function applyInitialTheme(): void {
  const storedTheme = readStoredThemePreference();
  applyResolvedTheme(resolveTheme(storedTheme));
}

export function useTheme(): void {
  const theme = useSettingsStore((state) => state.theme);

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      applyResolvedTheme(resolveTheme(theme));
      return;
    }

    const mediaQuery = window.matchMedia(SYSTEM_DARK_QUERY);
    const apply = () => {
      const resolved = theme === 'system' ? (mediaQuery.matches ? 'dark' : 'light') : theme;
      applyResolvedTheme(resolved);
    };

    apply();

    if (theme !== 'system') {
      return;
    }

    const handleChange = () => apply();

    if (typeof mediaQuery.addEventListener === 'function') {
      mediaQuery.addEventListener('change', handleChange);
      return () => mediaQuery.removeEventListener('change', handleChange);
    }

    mediaQuery.addListener(handleChange);
    return () => mediaQuery.removeListener(handleChange);
  }, [theme]);
}
