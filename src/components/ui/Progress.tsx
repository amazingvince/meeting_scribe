/**
 * Progress bar and spinner components
 */

import { Loader2 } from 'lucide-react';

interface ProgressBarProps {
  value: number;
  max?: number;
  size?: 'sm' | 'md' | 'lg';
  color?: 'indigo' | 'green' | 'blue' | 'red';
  showLabel?: boolean;
  label?: string;
}

const sizeStyles = {
  sm: 'h-1',
  md: 'h-2',
  lg: 'h-3',
};

const colorStyles = {
  indigo: 'bg-indigo-600',
  green: 'bg-green-600',
  blue: 'bg-blue-600',
  red: 'bg-red-600',
};

export function ProgressBar({
  value,
  max = 100,
  size = 'md',
  color = 'indigo',
  showLabel = false,
  label,
}: ProgressBarProps) {
  const percentage = Math.min(Math.max((value / max) * 100, 0), 100);

  return (
    <div className="w-full">
      {(showLabel || label) && (
        <div className="flex justify-between mb-1">
          <span className="text-sm text-muted-foreground">
            {label}
          </span>
          {showLabel && (
            <span className="text-sm font-medium text-foreground">
              {Math.round(percentage)}%
            </span>
          )}
        </div>
      )}
      <div
        className={`w-full bg-muted rounded-full overflow-hidden ${sizeStyles[size]}`}
      >
        <div
          className={`${colorStyles[color]} ${sizeStyles[size]} rounded-full transition-all duration-300 ease-out`}
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  );
}

interface SpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const spinnerSizes = {
  sm: 'w-4 h-4',
  md: 'w-6 h-6',
  lg: 'w-8 h-8',
};

export function Spinner({ size = 'md', className = '' }: SpinnerProps) {
  return (
    <Loader2
      className={`animate-spin text-primary ${spinnerSizes[size]} ${className}`}
    />
  );
}

interface LoadingOverlayProps {
  message?: string;
}

export function LoadingOverlay({ message }: LoadingOverlayProps) {
  return (
    <div className="absolute inset-0 bg-background/80 flex items-center justify-center z-10">
      <div className="flex flex-col items-center gap-3">
        <Spinner size="lg" />
        {message && (
          <p className="text-sm text-muted-foreground">{message}</p>
        )}
      </div>
    </div>
  );
}
