/**
 * Badge component for status indicators
 */

import type { ReactNode } from 'react';

interface BadgeProps {
  children: ReactNode;
  variant?: 'default' | 'success' | 'warning' | 'error' | 'info';
  size?: 'sm' | 'md';
  className?: string;
}

const variantStyles = {
  default: 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300',
  success: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
  warning: 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400',
  error: 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400',
  info: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400',
};

const sizeStyles = {
  sm: 'px-2 py-0.5 text-xs',
  md: 'px-2.5 py-1 text-sm',
};

export function Badge({
  children,
  variant = 'default',
  size = 'sm',
  className = '',
}: BadgeProps) {
  return (
    <span
      className={`
        inline-flex items-center font-medium rounded-full
        ${variantStyles[variant]}
        ${sizeStyles[size]}
        ${className}
      `}
    >
      {children}
    </span>
  );
}

/** Status badge with dot indicator */
interface StatusBadgeProps {
  status: 'recording' | 'processing' | 'ready' | 'archived' | 'error';
  className?: string;
}

const statusConfig = {
  recording: { color: 'bg-red-500', text: 'Recording', variant: 'error' as const },
  processing: { color: 'bg-yellow-500', text: 'Processing', variant: 'warning' as const },
  ready: { color: 'bg-green-500', text: 'Ready', variant: 'success' as const },
  archived: { color: 'bg-gray-500', text: 'Archived', variant: 'default' as const },
  error: { color: 'bg-red-500', text: 'Error', variant: 'error' as const },
};

export function StatusBadge({ status, className = '' }: StatusBadgeProps) {
  const config = statusConfig[status];

  return (
    <Badge variant={config.variant} className={className}>
      <span className={`w-1.5 h-1.5 rounded-full ${config.color} mr-1.5`} />
      {config.text}
    </Badge>
  );
}
