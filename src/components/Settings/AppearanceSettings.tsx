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
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent/70">
          <Sun className="w-5 h-5 text-muted-foreground" />
        </div>
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
                  ? 'border-foreground/30 bg-accent text-foreground'
                  : 'border-border bg-card text-foreground hover:bg-accent/50',
              ].join(' ')}
              aria-pressed={selected}
            >
              <div className="flex items-center gap-2 text-sm font-medium">
                {option.icon}
                {option.label}
              </div>
              <p className="mt-1 text-xs text-muted-foreground">
                {option.description}
              </p>
            </button>
          );
        })}
      </div>
    </Card>
  );
}
