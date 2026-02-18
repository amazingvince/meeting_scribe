/**
 * Library view - Meeting list with search and filters
 */

import { useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { RefreshCw, Sparkles } from 'lucide-react';
import { useMeetings } from '../../hooks';
import { Button } from '../ui/Button';
import { Modal, ModalFooter } from '../ui/Modal';
import { SkeletonMeetingCard } from '../ui/Skeleton';
import { NoMeetingsEmpty, NoSearchResultsEmpty } from '../ui/EmptyState';
import { MeetingCard } from './MeetingCard';
import { MeetingSearch } from './MeetingSearch';
import { ModelSelector } from './ModelSelector';
import {
  TimelineGroup,
  groupMeetingsByDate,
  groupLabels,
  type GroupedMeetings,
} from './TimelineGroup';
import type { Meeting } from '../../types';

export function LibraryView() {
  const navigate = useNavigate();
  const {
    meetings,
    isLoading,
    error,
    searchQuery,
    searchMatches,
    search,
    fetchMeetings,
    deleteMeeting,
  } = useMeetings();

  const [deleteTarget, setDeleteTarget] = useState<Meeting | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  const handleMeetingClick = useCallback(
    (meeting: Meeting) => {
      navigate(`/meeting/${meeting.id}`);
    },
    [navigate]
  );

  const handleDeleteClick = useCallback((meeting: Meeting) => {
    setDeleteTarget(meeting);
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    if (!deleteTarget) return;

    setIsDeleting(true);
    await deleteMeeting(deleteTarget.id);
    setIsDeleting(false);
    setDeleteTarget(null);
  }, [deleteTarget, deleteMeeting]);

  const handleCancelDelete = useCallback(() => {
    setDeleteTarget(null);
  }, []);

  const handleStartRecording = useCallback(() => {
    navigate('/');
  }, [navigate]);

  // Group meetings by date
  const groupedMeetings = groupMeetingsByDate(meetings);

  // Check if there are any meetings to display
  const hasMeetings = meetings.length > 0;
  const hasResults = Object.values(groupedMeetings).some(
    (group) => group.length > 0
  );

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 leading-tight">
          Meeting Library
        </h1>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => fetchMeetings()}
          disabled={isLoading}
        >
          <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
        </Button>
      </div>

      <div className="sticky top-0 z-20 -mx-2 px-2 py-2 bg-surface-50/95 dark:bg-surface-950/95 backdrop-blur border-b border-gray-200/70 dark:border-gray-800/70">
        <MeetingSearch
          value={searchQuery}
          onChange={search}
          placeholder="Search transcript (hybrid keyword + semantic)..."
        />
        <div className="mt-2 flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
          <span className="inline-flex items-center gap-1">
            <Sparkles className="w-3.5 h-3.5" />
            Hybrid transcript search
          </span>
          {searchQuery.trim().length > 0 && (
            <span>{meetings.length} result{meetings.length === 1 ? '' : 's'}</span>
          )}
        </div>
      </div>

      {/* Model Selector */}
      <ModelSelector />

      {/* Error state */}
      {error && (
        <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-400">
          {error}
        </div>
      )}

      {/* Loading state */}
      {isLoading && !hasMeetings && (
        <div className="space-y-3">
          <SkeletonMeetingCard />
          <SkeletonMeetingCard />
          <SkeletonMeetingCard />
        </div>
      )}

      {/* Empty states */}
      {!isLoading && !hasMeetings && !searchQuery && (
        <NoMeetingsEmpty onRecord={handleStartRecording} />
      )}

      {!isLoading && !hasResults && searchQuery && <NoSearchResultsEmpty />}

      {/* Meeting list grouped by date */}
      {hasResults && (
        <div className="space-y-6">
          {(Object.keys(groupLabels) as Array<keyof GroupedMeetings>).map(
            (key) => {
              const group = groupedMeetings[key];
              if (group.length === 0) return null;

              return (
                <TimelineGroup key={key} label={groupLabels[key]}>
                  {group.map((meeting) => (
                    <MeetingCard
                      key={meeting.id}
                      meeting={meeting}
                      searchQuery={searchQuery}
                      searchMatch={searchMatches[meeting.id]}
                      onClick={() => handleMeetingClick(meeting)}
                      onDelete={() => handleDeleteClick(meeting)}
                    />
                  ))}
                </TimelineGroup>
              );
            }
          )}
        </div>
      )}

      {/* Delete confirmation modal */}
      <Modal
        isOpen={!!deleteTarget}
        onClose={handleCancelDelete}
        title="Delete Meeting"
        size="sm"
      >
        <p className="text-gray-600 dark:text-gray-400">
          Are you sure you want to delete "{deleteTarget?.title}"? This action
          cannot be undone.
        </p>
        <ModalFooter>
          <Button variant="secondary" onClick={handleCancelDelete}>
            Cancel
          </Button>
          <Button
            variant="danger"
            onClick={handleConfirmDelete}
            isLoading={isDeleting}
          >
            Delete
          </Button>
        </ModalFooter>
      </Modal>
    </div>
  );
}
