/**
 * Meetings hook
 * Provides meeting list access and CRUD operations
 */

import { useCallback, useEffect } from 'react';
import { useMeetingsStore } from '../stores';
import type { Meeting, MeetingStatus } from '../types';

interface UseMeetingsOptions {
  autoFetch?: boolean;
}

export function useMeetings(options: UseMeetingsOptions = {}) {
  const { autoFetch = true } = options;
  const store = useMeetingsStore();
  const { fetchMeetings, searchQuery, statusFilter } = store;

  // Fetch on mount and whenever filters change
  useEffect(() => {
    if (autoFetch) {
      fetchMeetings();
    }
  }, [searchQuery, statusFilter, autoFetch, fetchMeetings]);

  const selectMeeting = useCallback(
    async (meeting: Meeting | null) => {
      store.selectMeeting(meeting);
      if (meeting) {
        await store.fetchTranscript(meeting.id);
      }
    },
    [store]
  );

  const search = useCallback(
    (query: string) => {
      store.setSearchQuery(query);
    },
    [store]
  );

  const filterByStatus = useCallback(
    (status: MeetingStatus | null) => {
      store.setStatusFilter(status);
    },
    [store]
  );

  return {
    // State
    meetings: store.meetings,
    selectedMeeting: store.selectedMeeting,
    selectedTranscript: store.selectedTranscript,
    isLoading: store.isLoading,
    isLoadingTranscript: store.isLoadingTranscript,
    error: store.error,
    searchQuery: store.searchQuery,
    searchMatches: store.searchMatches,
    statusFilter: store.statusFilter,

    // Actions
    fetchMeetings: store.fetchMeetings,
    fetchMeeting: store.fetchMeeting,
    selectMeeting,
    createMeeting: store.createMeeting,
    updateMeeting: store.updateMeeting,
    deleteMeeting: store.deleteMeeting,
    search,
    filterByStatus,
    clearError: store.clearError,
  };
}

/**
 * Hook for a single meeting
 */
export function useMeeting(meetingId: string | null) {
  const store = useMeetingsStore();
  const { fetchMeeting, selectMeeting, fetchTranscript } = store;

  useEffect(() => {
    let cancelled = false;

    if (!meetingId) {
      selectMeeting(null);
      return () => {
        cancelled = true;
      };
    }

    // Clear any previously-selected meeting while loading the new one.
    selectMeeting(null);

    fetchMeeting(meetingId).then((meeting) => {
      if (cancelled) return;
      if (meeting) {
        selectMeeting(meeting);
        fetchTranscript(meetingId);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [meetingId, fetchMeeting, selectMeeting, fetchTranscript]);

  return {
    meeting: store.selectedMeeting,
    transcript: store.selectedTranscript,
    isLoading: store.isLoading,
    isLoadingTranscript: store.isLoadingTranscript,
    error: store.error,
    updateMeeting: store.updateMeeting,
    deleteMeeting: store.deleteMeeting,
  };
}
