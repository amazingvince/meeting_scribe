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

  const navigateToMeeting = useCallback(
    (meetingId: string, startMs?: number | null) => {
      const params = new URLSearchParams();
      if (typeof startMs === 'number' && Number.isFinite(startMs) && startMs >= 0) {
        params.set('t', String(Math.round(startMs)));
      }

      const search = params.toString();
      navigate({
        pathname: `/meeting/${meetingId}`,
        search: search ? `?${search}` : '',
      });
    },
    [navigate]
  );

  const handleMeetingClick = useCallback(
    (meeting: Meeting) => {
      navigateToMeeting(meeting.id);
    },
    [navigateToMeeting]
  );

  const handleJumpToMatch = useCallback(
    (meeting: Meeting, startMs: number) => {
      navigateToMeeting(meeting.id, startMs);
    },
    [navigateToMeeting]
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
    <div className="flex flex-col h-full">
      {/* Header */}
      <header className="px-6 py-4 border-b border-border bg-card">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-foreground">Past Meetings</h2>
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">
              {searchQuery.trim().length > 0
                ? `${meetings.length} result${meetings.length === 1 ? '' : 's'}`
                : `${meetings.length} meeting${meetings.length === 1 ? '' : 's'}`}
            </span>
            <Button
              variant="outline"
              size="icon"
              onClick={() => fetchMeetings()}
              disabled={isLoading}
              className="h-8 w-8"
              aria-label="Refresh meetings"
            >
              <RefreshCw className={`h-3.5 w-3.5 ${isLoading ? 'animate-spin' : ''}`} />
            </Button>
          </div>
        </div>
        <MeetingSearch
          value={searchQuery}
          onChange={search}
          placeholder="Search meetings, transcripts..."
        />
      </header>

      {error && (
        <div className="border-b border-destructive/30 bg-destructive/5 px-6 py-2">
          <p className="text-sm text-destructive">{error}</p>
        </div>
      )}

      {/* Meeting list */}
      <div className="no-scrollbar flex-1 min-h-0 overflow-y-auto">
        <div className="p-4 pb-20 md:pb-4 space-y-1">
          {isLoading && !hasMeetings && (
            <div className="space-y-3 p-2">
              <SkeletonMeetingCard />
              <SkeletonMeetingCard />
              <SkeletonMeetingCard />
            </div>
          )}

          {!isLoading && !hasMeetings && !searchQuery && (
            <NoMeetingsEmpty onRecord={handleStartRecording} />
          )}

          {!isLoading && !hasResults && searchQuery && <NoSearchResultsEmpty />}

          {hasResults &&
            (Object.keys(groupLabels) as Array<keyof GroupedMeetings>).map((key) => {
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
                      onJumpToTimestamp={(startMs) => handleJumpToMatch(meeting, startMs)}
                      onDelete={() => handleDeleteClick(meeting)}
                    />
                  ))}
                </TimelineGroup>
              );
            })}
        </div>
      </div>

      <Modal
        isOpen={!!deleteTarget}
        onClose={handleCancelDelete}
        title="Delete Meeting"
        size="sm"
      >
        <p className="text-muted-foreground">
          Are you sure you want to delete "{deleteTarget?.title}"? This action
          cannot be undone.
        </p>
        <ModalFooter>
          <Button variant="secondary" onClick={handleCancelDelete}>
            Cancel
          </Button>
          <Button
            variant="destructive"
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
