/**
 * Styled select primitive matching the design system
 */

import type { SelectHTMLAttributes } from 'react';
import { cn } from '../../lib/utils';

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  error?: string;
}

export function Select({ label, error, className, id, ...props }: SelectProps) {
  return (
    <div>
      {label && (
        <label
          htmlFor={id}
          className="mb-2 block text-sm font-medium text-foreground"
        >
          {label}
        </label>
      )}
      <select
        id={id}
        className={cn(
          'w-full rounded-md border border-input bg-input-background px-3 py-2 text-sm text-foreground',
          'focus:outline-none focus:ring-2 focus:ring-ring/20',
          'disabled:cursor-not-allowed disabled:opacity-50',
          error && 'border-destructive',
          className
        )}
        {...props}
      />
      {error && (
        <p className="mt-1 text-xs text-destructive">{error}</p>
      )}
    </div>
  );
}
