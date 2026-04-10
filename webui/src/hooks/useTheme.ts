import { useEffect, useState } from 'react';

const THEME_STORAGE_KEY = 'box4:webui:theme';

export type ThemeMode = 'system' | 'light' | 'dark';

function readStoredTheme(): ThemeMode {
  if (typeof window === 'undefined') return 'system';
  const value = window.localStorage.getItem(THEME_STORAGE_KEY);
  return value === 'light' || value === 'dark' || value === 'system' ? value : 'system';
}

function readSystemIsDark() {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

export function useTheme() {
  const [theme, setTheme] = useState<ThemeMode>(readStoredTheme);
  const [systemIsDark, setSystemIsDark] = useState(readSystemIsDark);

  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    setSystemIsDark(mq.matches);
    const handler = (e: MediaQueryListEvent | MediaQueryList) => setSystemIsDark(e.matches);

    if (typeof mq.addEventListener === 'function') {
      mq.addEventListener('change', handler);
      return () => mq.removeEventListener('change', handler);
    }

    mq.addListener(handler);
    return () => mq.removeListener(handler);
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  const isDark = theme === 'system' ? systemIsDark : theme === 'dark';

  useEffect(() => {
    if (typeof document === 'undefined') return;
    const root = document.documentElement;
    root.classList.toggle('dark', isDark);
    root.style.colorScheme = isDark ? 'dark' : 'light';
  }, [isDark]);

  const cycleTheme = () => {
    if (theme === 'system') setTheme('light');
    else if (theme === 'light') setTheme('dark');
    else setTheme('system');
  };

  return {
    theme,
    isDark,
    cycleTheme,
    setTheme,
  };
}
