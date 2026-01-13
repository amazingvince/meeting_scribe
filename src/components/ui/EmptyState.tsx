/**
 * Empty state component for empty lists
 */

import type { ReactNode } from 'react';
import { FileQuestion, Search, MessageSquare, Mic } from 'lucide-react';
import { Button } from './Button';

interface EmptyStateProps {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: {
    label: string;
    onClick: () => void;
  };
  className?: string;
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className = '',
}: EmptyStateProps) {
  return (
    <div
      className={`
        flex flex-col items-center justify-center
        py-12 px-4 text-center
        ${className}
      `}
    >
      {icon && (
        <div className="mb-4 text-gray-400 dark:text-gray-500">{icon}</div>
      )}
      <h3 className="text-lg font-medium text-gray-900 dark:text-gray-100 mb-1">
        {title}
      </h3>
      {description && (
        <p className="text-sm text-gray-500 dark:text-gray-400 max-w-sm mb-4">
          {description}
        </p>
      )}
      {action && (
        <Button onClick={action.onClick} variant="primary">
          {action.label}
        </Button>
      )}
    </div>
  );
}

export function NoMeetingsEmpty({ onRecord }: { onRecord: () => void }) {
  return (
    <EmptyState
      icon={<Mic className="w-12 h-12" />}
      title="No meetings yet"
      description="Start recording your first meeting to see it here."
      action={{ label: 'Start Recording', onClick: onRecord }}
    />
  );
}

export function NoSearchResultsEmpty() {
  return (
    <EmptyState
      icon={<Search className="w-12 h-12" />}
      title="No results found"
      description="Try adjusting your search terms or filters."
    />
  );
}

export function NoChatMessagesEmpty() {
  return (
    <EmptyState
      icon={<MessageSquare className="w-12 h-12" />}
      title="Start a conversation"
      description="Ask questions about your meetings and I'll help you find the answers."
    />
  );
}

export function NoTranscriptEmpty() {
  return (
    <EmptyState
      icon={<FileQuestion className="w-12 h-12" />}
      title="No transcript available"
      description="This meeting hasn't been transcribed yet."
    />
  );
}
