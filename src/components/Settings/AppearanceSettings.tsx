/**
 * Appearance settings section
 */

import { Monitor, Moon, Sun } from 'lucide-react';
import { Card, CardTitle } from '../ui/Card';
import { useSettingsStore } from '../../stores';
import type { ReactNode } from 'react';

type ThemeOption = {
  value: 'light' | 'dark' | 'system';
  label: string;
  description: string;
  icon: ReactNode;
};

const THEME_OPTIONS: ThemeOption[] = [
  {
    value: 'system',
    label: 'System',
    description: 'Use your operating system preference.',
    icon: <Monitor className="w-4 h-4" />,
  },
  {
    value: 'light',
    label: 'Light',
    description: 'Always use light appearance.',
    icon: <Sun className="w-4 h-4" />,
  },
  {
    value: 'dark',
    label: 'Dark',
    description: 'Always use dark appearance.',
    icon: <Moon className="w-4 h-4" />,
  },
];

export function AppearanceSettings() {
  const { theme, setTheme } = useSettingsStore();

  return (
    <Card>
      <div className="flex items-center gap-2 mb-4">
        <Sun className="w-5 h-5 text-amber-500" />
        <CardTitle>Appearance</CardTitle>
      </div>

      <div className="grid gap-2 sm:grid-cols-3">
        {THEME_OPTIONS.map((option) => {
          const selected = theme === option.value;
          return (
            <button
              key={option.value}
              type="button"
              onClick={() => setTheme(option.value)}
              className={[
                'rounded-lg border p-3 text-left transition-colors',
                selected
                  ? 'border-primary-500 bg-primary-50 text-primary-800 dark:border-primary-400 dark:bg-primary-900/20 dark:text-primary-200'
                  : 'border-gray-200 bg-white text-gray-700 hover:border-gray-300 hover:bg-gray-50 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-200 dark:hover:border-gray-600 dark:hover:bg-gray-700/70',
              ].join(' ')}
              aria-pressed={selected}
            >
              <div className="flex items-center gap-2 text-sm font-medium">
                {option.icon}
                {option.label}
              </div>
              <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                {option.description}
              </p>
            </button>
          );
        })}
      </div>
    </Card>
  );
}
