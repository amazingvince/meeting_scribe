/**
 * Meeting card component for library list
 */

import { Calendar, Clock, Trash2, MoreVertical, Search } from 'lucide-react';
import { motion } from 'framer-motion';
import type { Meeting } from '../../types';
import { Card } from '../ui/Card';
import { StatusBadge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { formatDuration, formatRelativeDate } from '../../utils/format';
import { useState, useEffect, type ReactNode } from 'react';

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
      <mark className="rounded bg-highlight/60 px-0.5 text-inherit">
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

  useEffect(() => {
    if (!showMenu) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setShowMenu(false);
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [showMenu]);

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete();
  };

  const handleMenuToggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    setShowMenu(!showMenu);
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.1, ease: [0.22, 1, 0.36, 1] }}
      whileHover={{ y: -0.5 }}
    >
      <Card
        hover
        onClick={onClick}
        className="group relative shadow-none"
      >
        <div className="flex items-start justify-between">
          <div className="min-w-0 flex-1">
            <h3 className="truncate font-semibold text-foreground">
              {meeting.title}
            </h3>
            <div className="mt-1 flex items-center gap-4 text-sm text-muted-foreground">
              <span className="flex items-center gap-1">
                <Calendar className="h-4 w-4" />
                {formatRelativeDate(meeting.created_at)}
              </span>
              {meeting.duration_ms && (
                <span className="flex items-center gap-1">
                  <Clock className="h-4 w-4" />
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
                className="h-8 w-8 opacity-70 group-hover:opacity-100"
              >
                <MoreVertical className="h-4 w-4" />
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
                  <div className="absolute right-0 top-full z-20 mt-1 min-w-[120px] rounded-lg border border-border bg-card py-1 shadow-float">
                    <button
                      className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-destructive hover:bg-destructive/5"
                      onClick={handleDelete}
                    >
                      <Trash2 className="h-4 w-4" />
                      Delete
                    </button>
                  </div>
                </>
              )}
            </div>
          </div>
        </div>

        {meeting.error_message && meeting.status === 'error' && (
          <p className="mt-2 text-sm text-destructive truncate">
            {meeting.error_message}
          </p>
        )}

        {searchMatch && searchQuery.trim().length > 0 && (
          <div className="mt-3 rounded-lg border border-border bg-muted/50 px-3 py-2">
            <div className="flex items-center gap-2 text-[11px] uppercase tracking-wide text-muted-foreground">
              <Search className="h-3.5 w-3.5" />
              <span>
                {searchMatch.source === 'hybrid' ? 'Transcript match' : 'Title match'}
                {searchMatch.startMs !== null ? ` • ${formatDuration(searchMatch.startMs)}` : ''}
              </span>
            </div>
            <p className="mt-1 text-sm text-foreground line-clamp-2">
              {renderSnippet(searchMatch.snippet, searchQuery)}
            </p>
          </div>
        )}
      </Card>
    </motion.div>
  );
}
