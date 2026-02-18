/**
 * Meeting card component for library list
 */

import { Calendar, Clock, Trash2, MoreVertical, Search } from 'lucide-react';
import type { Meeting } from '../../types';
import { Card } from '../ui/Card';
import { StatusBadge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { formatDuration, formatRelativeDate } from '../../utils/format';
import { useState, type ReactNode } from 'react';

interface MeetingSearchMatch {
  snippet: string;
  startMs: number | null;
  source: 'hybrid' | 'title';
}

interface MeetingCardProps {
  meeting: Meeting;
  searchQuery?: string;
  searchMatch?: MeetingSearchMatch;
  onClick: () => void;
  onDelete: () => void;
}

function renderSnippet(snippet: string, query: string): ReactNode {
  const normalizedQuery = query.trim();
  if (!normalizedQuery) {
    return snippet;
  }

  const lowerSnippet = snippet.toLowerCase();
  const lowerQuery = normalizedQuery.toLowerCase();
  const startIndex = lowerSnippet.indexOf(lowerQuery);
  if (startIndex < 0) {
    return snippet;
  }

  const endIndex = startIndex + normalizedQuery.length;
  return (
    <>
      {snippet.slice(0, startIndex)}
      <mark className="rounded bg-amber-200/80 dark:bg-amber-400/30 px-0.5 text-inherit">
        {snippet.slice(startIndex, endIndex)}
      </mark>
      {snippet.slice(endIndex)}
    </>
  );
}

export function MeetingCard({
  meeting,
  searchQuery = '',
  searchMatch,
  onClick,
  onDelete,
}: MeetingCardProps) {
  const [showMenu, setShowMenu] = useState(false);

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete();
  };

  const handleMenuToggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    setShowMenu(!showMenu);
  };

  return (
    <Card hover onClick={onClick} className="relative group">
      <div className="flex justify-between items-start">
        <div className="flex-1 min-w-0">
          <h3 className="font-medium text-gray-900 dark:text-gray-100 truncate">
            {meeting.title}
          </h3>
          <div className="flex items-center gap-4 mt-1 text-sm text-gray-500 dark:text-gray-400">
            <span className="flex items-center gap-1">
              <Calendar className="w-4 h-4" />
              {formatRelativeDate(meeting.created_at)}
            </span>
            {meeting.duration_ms && (
              <span className="flex items-center gap-1">
                <Clock className="w-4 h-4" />
                {formatDuration(meeting.duration_ms)}
              </span>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <StatusBadge status={meeting.status} />
          <div className="relative">
            <Button
              variant="ghost"
              size="icon"
              onClick={handleMenuToggle}
              className="opacity-0 group-hover:opacity-100"
            >
              <MoreVertical className="w-4 h-4" />
            </Button>

            {showMenu && (
              <>
                <div
                  className="fixed inset-0 z-10"
                  onClick={(e) => {
                    e.stopPropagation();
                    setShowMenu(false);
                  }}
                />
                <div className="absolute right-0 top-full mt-1 z-20 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg py-1 min-w-[120px]">
                  <button
                    className="w-full px-4 py-2 text-left text-sm text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 flex items-center gap-2"
                    onClick={handleDelete}
                  >
                    <Trash2 className="w-4 h-4" />
                    Delete
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      {meeting.error_message && meeting.status === 'error' && (
        <p className="mt-2 text-sm text-red-600 truncate">
          {meeting.error_message}
        </p>
      )}

      {searchMatch && searchQuery.trim().length > 0 && (
        <div className="mt-3 rounded-md border border-indigo-100 dark:border-indigo-900/60 bg-indigo-50/70 dark:bg-indigo-900/20 px-3 py-2">
          <div className="flex items-center gap-2 text-[11px] uppercase tracking-wide text-indigo-600 dark:text-indigo-300">
            <Search className="w-3.5 h-3.5" />
            <span>
              {searchMatch.source === 'hybrid' ? 'Transcript match' : 'Title match'}
              {searchMatch.startMs !== null ? ` • ${formatDuration(searchMatch.startMs)}` : ''}
            </span>
          </div>
          <p className="mt-1 text-sm text-indigo-900 dark:text-indigo-200 line-clamp-2">
            {renderSnippet(searchMatch.snippet, searchQuery)}
          </p>
        </div>
      )}
    </Card>
  );
}
