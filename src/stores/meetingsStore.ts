/**
 * Meetings state store
 * Manages meeting list cache, selected meeting, and CRUD operations
 */

import { create } from 'zustand';
import type {
  Meeting,
  MeetingStatus,
  SemanticSearchResult,
  TranscriptSegment,
} from '../types';
import * as api from '../lib/tauri';
import { modelManager } from '../lib/modelManager';

const DEFAULT_LIST_LIMIT = 100;
const HYBRID_SEARCH_LIMIT = 120;
const HYBRID_SEARCH_CANDIDATE_LIMIT = 300;
const MIN_HYBRID_QUERY_CHARS = 2;

function compactSnippet(text: string, maxChars = 220): string {
  const compact = text.replace(/\s+/g, ' ').trim();
  if (compact.length <= maxChars) {
    return compact;
  }
  return `${compact.slice(0, maxChars - 1).trimEnd()}…`;
}

function titleMatchBoost(query: string, title: string): number {
  const normalizedQuery = query.trim().toLowerCase();
  const normalizedTitle = title.toLowerCase();
  if (!normalizedQuery || !normalizedTitle.includes(normalizedQuery)) {
    return 0;
  }

  // Favor stronger title matches while keeping transcript-based ranking dominant.
  const coverage = normalizedQuery.length / Math.max(normalizedTitle.length, 8);
  return 0.08 + Math.min(coverage, 0.24);
}

export interface MeetingSearchMatch {
  snippet: string;
  startMs: number | null;
  source: 'hybrid' | 'title';
}

interface RankedMeeting {
  meeting: Meeting;
  score: number;
  match: MeetingSearchMatch;
}

function rankHybridHit(hit: SemanticSearchResult, rankIndex: number): number {
  const rankScore = 1 / (rankIndex + 1);
  const similarityScore = Number.isFinite(hit.similarity)
    ? Math.max(0, hit.similarity)
    : 0;
  const sourceBoost = hit.chunk_type === 'fts' ? 0.1 : 0;
  return rankScore + similarityScore * 0.35 + sourceBoost;
}

interface MeetingsStore {
  // State
  meetings: Meeting[];
  searchMatches: Record<string, MeetingSearchMatch>;
  selectedMeeting: Meeting | null;
  selectedTranscript: TranscriptSegment[];
  isLoading: boolean;
  isLoadingTranscript: boolean;
  error: string | null;
  searchQuery: string;
  statusFilter: MeetingStatus | null;
  meetingsRequestSeq: number;

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
  searchMatches: {},
  selectedMeeting: null,
  selectedTranscript: [],
  isLoading: false,
  isLoadingTranscript: false,
  error: null,
  searchQuery: '',
  statusFilter: null,
  meetingsRequestSeq: 0,

  fetchMeetings: async () => {
    const requestSeq = get().meetingsRequestSeq + 1;
    const { searchQuery, statusFilter } = get();
    set({ isLoading: true, error: null, meetingsRequestSeq: requestSeq });

    try {
      const trimmedQuery = searchQuery.trim();
      const hasSearch = trimmedQuery.length > 0;
      let meetings: Meeting[] = [];
      let searchMatches: Record<string, MeetingSearchMatch> = {};

      if (!hasSearch) {
        meetings = await api.listMeetings({
          status: statusFilter ?? undefined,
          limit: DEFAULT_LIST_LIMIT,
        });
      } else if (trimmedQuery.length < MIN_HYBRID_QUERY_CHARS) {
        const candidateMeetings = await api.listMeetings({
          status: statusFilter ?? undefined,
          limit: DEFAULT_LIST_LIMIT,
        });
        const normalizedQuery = trimmedQuery.toLowerCase();
        const ranked = candidateMeetings
          .filter((meeting) => meeting.title.toLowerCase().includes(normalizedQuery))
          .map((meeting) => ({
            meeting,
            score: titleMatchBoost(trimmedQuery, meeting.title),
            match: {
              snippet: compactSnippet(meeting.title),
              startMs: null,
              source: 'title' as const,
            },
          }))
          .sort(
            (a, b) => b.score - a.score || b.meeting.created_at - a.meeting.created_at
          );

        meetings = ranked.map((entry) => entry.meeting);
        searchMatches = Object.fromEntries(
          ranked.map((entry) => [entry.meeting.id, entry.match])
        );
      } else {
        const embeddingReady = await modelManager.ensureEmbeddingReady();
        if (!embeddingReady) {
          throw new Error(
            'Embedding model is not ready. Download and load it in Settings to use hybrid search.'
          );
        }

        const [candidateMeetings, hybridResults] = await Promise.all([
          api.listMeetings({
            status: statusFilter ?? undefined,
            limit: HYBRID_SEARCH_CANDIDATE_LIMIT,
          }),
          api.hybridSearch(trimmedQuery, HYBRID_SEARCH_LIMIT),
        ]);

        const rankedByMeetingId = new Map<string, RankedMeeting>();
        const meetingById = new Map<string, Meeting>();
        for (const meeting of candidateMeetings) {
          meetingById.set(meeting.id, meeting);
        }

        for (const [rankIndex, hit] of hybridResults.entries()) {
          const meeting = meetingById.get(hit.meeting_id);
          if (!meeting) {
            continue;
          }

          const nextScore = rankHybridHit(hit, rankIndex);
          const existing = rankedByMeetingId.get(hit.meeting_id);
          const match: MeetingSearchMatch = {
            snippet: compactSnippet(hit.text),
            startMs: hit.start_ms ?? null,
            source: 'hybrid',
          };

          if (!existing || nextScore > existing.score) {
            rankedByMeetingId.set(hit.meeting_id, {
              meeting,
              score: nextScore,
              match,
            });
          }
        }

        for (const meeting of candidateMeetings) {
          const boost = titleMatchBoost(trimmedQuery, meeting.title);
          if (boost <= 0) {
            continue;
          }

          const existing = rankedByMeetingId.get(meeting.id);
          if (existing) {
            existing.score += boost;
            continue;
          }

          rankedByMeetingId.set(meeting.id, {
            meeting,
            score: boost,
            match: {
              snippet: compactSnippet(meeting.title),
              startMs: null,
              source: 'title',
            },
          });
        }

        const ranked = [...rankedByMeetingId.values()].sort(
          (a, b) => b.score - a.score || b.meeting.created_at - a.meeting.created_at
        );
        meetings = ranked.map((entry) => entry.meeting);
        searchMatches = Object.fromEntries(
          ranked.map((entry) => [entry.meeting.id, entry.match])
        );
      }

      set((state) => {
        if (state.meetingsRequestSeq !== requestSeq) {
          return {};
        }
        return { meetings, searchMatches, isLoading: false };
      });
    } catch (e) {
      set((state) => ({
        ...(state.meetingsRequestSeq === requestSeq
          ? {
              error: e instanceof Error ? e.message : String(e),
              isLoading: false,
            }
          : {}),
      }));
    }
  },

  fetchMeeting: async (id) => {
    set({ isLoading: true, error: null });
    try {
      const meeting = await api.getMeeting(id);
      if (meeting) {
        // Update in list if exists
        set((state) => ({
          meetings: state.meetings.map((m) => (m.id === id ? meeting : m)),
          selectedMeeting:
            state.selectedMeeting?.id === id ? meeting : state.selectedMeeting,
          isLoading: false,
        }));
      } else {
        set({ isLoading: false });
      }
      return meeting;
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
      });
      return null;
    }
  },

  fetchTranscript: async (meetingId) => {
    set({ isLoadingTranscript: true, error: null });
    try {
      const transcript = await api.getTranscript(meetingId);
      // Avoid race conditions when switching meetings quickly
      set((state) => {
        if (state.selectedMeeting?.id !== meetingId) {
          return { isLoadingTranscript: false };
        }
        return { selectedTranscript: transcript, isLoadingTranscript: false };
      });
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoadingTranscript: false,
        selectedTranscript: [],
      });
    }
  },

  selectMeeting: (meeting) => {
    set((state) => ({
      selectedMeeting: meeting,
      selectedTranscript:
        meeting && state.selectedMeeting?.id === meeting.id
          ? state.selectedTranscript
          : [],
    }));
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
          searchMatches: Object.fromEntries(
            Object.entries(state.searchMatches).filter(([meetingId]) => meetingId !== id)
          ),
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
