/**
 * Library view - Meeting list with search and filters
 */

import { useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { RefreshCw } from 'lucide-react';
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
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
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

      {/* Search */}
      <MeetingSearch value={searchQuery} onChange={search} />

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
