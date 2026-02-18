import { cva, type VariantProps } from 'class-variance-authority';
import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from '../../lib/utils';

const badgeVariants = cva(
  'inline-flex items-center rounded-full font-medium transition-colors',
  {
    variants: {
      variant: {
        default: 'bg-secondary text-secondary-foreground',
        secondary: 'bg-muted text-muted-foreground',
        outline: 'border border-border text-foreground',
        success:
          'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
        warning:
          'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400',
        error: 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400',
        info: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400',
      },
      size: {
        sm: 'px-2 py-0.5 text-xs',
        md: 'px-2.5 py-1 text-sm',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'sm',
    },
  }
);

interface BadgeProps
  extends HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {
  children: ReactNode;
}

export function Badge({ children, variant, size, className, ...props }: BadgeProps) {
  return (
    <span className={cn(badgeVariants({ variant, size }), className)} {...props}>
      {children}
    </span>
  );
}

interface StatusBadgeProps {
  status: 'recording' | 'processing' | 'ready' | 'archived' | 'error';
  className?: string;
}

const statusConfig = {
  recording: { color: 'bg-red-500', text: 'Recording', variant: 'error' as const },
  processing: {
    color: 'bg-yellow-500',
    text: 'Processing',
    variant: 'warning' as const,
  },
  ready: { color: 'bg-green-500', text: 'Ready', variant: 'success' as const },
  archived: { color: 'bg-muted-foreground', text: 'Archived', variant: 'secondary' as const },
  error: { color: 'bg-red-500', text: 'Error', variant: 'error' as const },
};

export function StatusBadge({ status, className }: StatusBadgeProps) {
  const config = statusConfig[status];
  return (
    <Badge variant={config.variant} className={className}>
      <span className={cn('mr-1.5 h-1.5 w-1.5 rounded-full', config.color)} />
      {config.text}
    </Badge>
  );
}
