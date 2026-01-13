/**
 * Meetings state store
 * Manages meeting list cache, selected meeting, and CRUD operations
 */

import { create } from 'zustand';
import type { Meeting, MeetingStatus, TranscriptSegment } from '../types';
import * as api from '../lib/tauri';

interface MeetingsStore {
  // State
  meetings: Meeting[];
  selectedMeeting: Meeting | null;
  selectedTranscript: TranscriptSegment[];
  isLoading: boolean;
  isLoadingTranscript: boolean;
  error: string | null;
  searchQuery: string;
  statusFilter: MeetingStatus | null;

  // Actions
  fetchMeetings: () => Promise<void>;
  fetchMeeting: (id: string) => Promise<Meeting | null>;
  fetchTranscript: (meetingId: string) => Promise<void>;
  selectMeeting: (meeting: Meeting | null) => void;
  createMeeting: (title?: string) => Promise<Meeting | null>;
  updateMeeting: (meeting: Meeting) => Promise<void>;
  deleteMeeting: (id: string) => Promise<boolean>;
  setSearchQuery: (query: string) => void;
  setStatusFilter: (status: MeetingStatus | null) => void;
  clearError: () => void;
}

export const useMeetingsStore = create<MeetingsStore>((set, get) => ({
  // Initial state
  meetings: [],
  selectedMeeting: null,
  selectedTranscript: [],
  isLoading: false,
  isLoadingTranscript: false,
  error: null,
  searchQuery: '',
  statusFilter: null,

  fetchMeetings: async () => {
    const { searchQuery, statusFilter } = get();
    set({ isLoading: true, error: null });

    try {
      const meetings = await api.listMeetings({
        status: statusFilter ?? undefined,
        search: searchQuery || undefined,
        limit: 100,
      });
      set({ meetings, isLoading: false });
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
      });
    }
  },

  fetchMeeting: async (id) => {
    try {
      const meeting = await api.getMeeting(id);
      if (meeting) {
        // Update in list if exists
        set((state) => ({
          meetings: state.meetings.map((m) => (m.id === id ? meeting : m)),
          selectedMeeting:
            state.selectedMeeting?.id === id ? meeting : state.selectedMeeting,
        }));
      }
      return meeting;
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      return null;
    }
  },

  fetchTranscript: async (meetingId) => {
    set({ isLoadingTranscript: true });
    try {
      const transcript = await api.getTranscript(meetingId);
      set({ selectedTranscript: transcript, isLoadingTranscript: false });
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoadingTranscript: false,
        selectedTranscript: [],
      });
    }
  },

  selectMeeting: (meeting) => {
    set({
      selectedMeeting: meeting,
      selectedTranscript: meeting ? get().selectedTranscript : [],
    });
  },

  createMeeting: async (title) => {
    try {
      const meeting = await api.createMeeting(title);
      set((state) => ({
        meetings: [meeting, ...state.meetings],
      }));
      return meeting;
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      return null;
    }
  },

  updateMeeting: async (meeting) => {
    try {
      await api.updateMeeting(meeting);
      set((state) => ({
        meetings: state.meetings.map((m) =>
          m.id === meeting.id ? meeting : m
        ),
        selectedMeeting:
          state.selectedMeeting?.id === meeting.id
            ? meeting
            : state.selectedMeeting,
      }));
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  deleteMeeting: async (id) => {
    try {
      const deleted = await api.deleteMeeting(id);
      if (deleted) {
        set((state) => ({
          meetings: state.meetings.filter((m) => m.id !== id),
          selectedMeeting:
            state.selectedMeeting?.id === id ? null : state.selectedMeeting,
          selectedTranscript:
            state.selectedMeeting?.id === id ? [] : state.selectedTranscript,
        }));
      }
      return deleted;
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  setSearchQuery: (query) => {
    set({ searchQuery: query });
  },

  setStatusFilter: (status) => {
    set({ statusFilter: status });
  },

  clearError: () => {
    set({ error: null });
  },
}));
