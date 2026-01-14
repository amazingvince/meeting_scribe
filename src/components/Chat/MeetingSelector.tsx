/**
 * Meeting selector for chat context
 * Allows users to filter chat to specific meetings
 */

import { useCallback, useEffect } from 'react';
import { X, Check, Calendar, Clock } from 'lucide-react';
import { useMeetings } from '../../hooks';
import type { Meeting } from '../../types';

interface MeetingSelectorProps {
  /** Currently selected meeting IDs */
  selectedIds: string[];
  /** Callback when selection changes */
  onSelect: (ids: string[]) => void;
}

/** Format relative time */
function formatRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (minutes < 1) return 'Just now';
  if (minutes < 60) return `${minutes}m ago`;
  if (hours < 24) return `${hours}h ago`;
  if (days < 7) return `${days}d ago`;
  return new Date(timestamp).toLocaleDateString();
}

/** Format duration */
function formatDuration(ms: number | null): string {
  if (!ms) return '';
  const minutes = Math.floor(ms / 60000);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}

export function MeetingSelector({ selectedIds, onSelect }: MeetingSelectorProps) {
  const { meetings, fetchMeetings, isLoading } = useMeetings({ autoFetch: false });

  // Fetch meetings on mount
  useEffect(() => {
    fetchMeetings();
  }, [fetchMeetings]);

  const toggleMeeting = useCallback(
    (meetingId: string) => {
      if (selectedIds.includes(meetingId)) {
        onSelect(selectedIds.filter((id) => id !== meetingId));
      } else {
        onSelect([...selectedIds, meetingId]);
      }
    },
    [selectedIds, onSelect]
  );

  const clearAll = useCallback(() => {
    onSelect([]);
  }, [onSelect]);

  // Get ready meetings only
  const readyMeetings = meetings.filter((m) => m.status === 'ready');

  if (isLoading) {
    return (
      <div className="px-4 py-2 text-sm text-gray-500 dark:text-gray-400">
        Loading meetings...
      </div>
    );
  }

  if (readyMeetings.length === 0) {
    return (
      <div className="px-4 py-2 text-sm text-gray-500 dark:text-gray-400">
        No meetings available. Record a meeting first.
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between px-2">
        <span className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          Filter by meeting
        </span>
        {selectedIds.length > 0 && (
          <button
            onClick={clearAll}
            className="text-xs text-indigo-600 dark:text-indigo-400 hover:underline"
          >
            Clear ({selectedIds.length})
          </button>
        )}
      </div>

      {/* Selected badges */}
      {selectedIds.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-2">
          {selectedIds.map((id) => {
            const meeting = readyMeetings.find((m) => m.id === id);
            if (!meeting) return null;
            return (
              <span
                key={id}
                className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300"
              >
                <span className="max-w-24 truncate">{meeting.title}</span>
                <button
                  onClick={() => toggleMeeting(id)}
                  className="hover:bg-indigo-200 dark:hover:bg-indigo-800 rounded-full p-0.5"
                >
                  <X className="w-3 h-3" />
                </button>
              </span>
            );
          })}
        </div>
      )}

      {/* Meeting list */}
      <div className="max-h-48 overflow-y-auto px-2 space-y-1">
        {readyMeetings.slice(0, 10).map((meeting) => (
          <MeetingItem
            key={meeting.id}
            meeting={meeting}
            selected={selectedIds.includes(meeting.id)}
            onToggle={toggleMeeting}
          />
        ))}
        {readyMeetings.length > 10 && (
          <p className="text-xs text-gray-400 dark:text-gray-500 text-center py-1">
            Showing 10 of {readyMeetings.length} meetings
          </p>
        )}
      </div>

      {/* Info text */}
      {selectedIds.length === 0 && (
        <p className="px-2 text-xs text-gray-400 dark:text-gray-500">
          Select meetings to search within, or search all meetings.
        </p>
      )}
    </div>
  );
}

interface MeetingItemProps {
  meeting: Meeting;
  selected: boolean;
  onToggle: (id: string) => void;
}

function MeetingItem({ meeting, selected, onToggle }: MeetingItemProps) {
  return (
    <button
      onClick={() => onToggle(meeting.id)}
      className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left transition-colors ${
        selected
          ? 'bg-indigo-50 dark:bg-indigo-900/30 border border-indigo-200 dark:border-indigo-700'
          : 'hover:bg-gray-100 dark:hover:bg-gray-800 border border-transparent'
      }`}
    >
      {/* Checkbox indicator */}
      <div
        className={`w-4 h-4 rounded border flex items-center justify-center flex-shrink-0 ${
          selected
            ? 'bg-indigo-600 border-indigo-600 text-white'
            : 'border-gray-300 dark:border-gray-600'
        }`}
      >
        {selected && <Check className="w-3 h-3" />}
      </div>

      {/* Meeting info */}
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
          {meeting.title}
        </div>
        <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
          <span className="flex items-center gap-0.5">
            <Calendar className="w-3 h-3" />
            {formatRelativeTime(meeting.created_at)}
          </span>
          {meeting.duration_ms && (
            <span className="flex items-center gap-0.5">
              <Clock className="w-3 h-3" />
              {formatDuration(meeting.duration_ms)}
            </span>
          )}
        </div>
      </div>
    </button>
  );
}
