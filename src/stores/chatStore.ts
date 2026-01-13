/**
 * Chat state store
 * Manages chat messages, streaming state, and RAG interactions
 */

import { create } from 'zustand';
import type { ChatMessage, ChatSource, SemanticSearchResult } from '../types';
import * as api from '../lib/tauri';

interface ChatStore {
  // State
  messages: ChatMessage[];
  isLoading: boolean;
  isStreaming: boolean;
  error: string | null;
  selectedMeetingIds: string[];

  // Actions
  sendMessage: (content: string) => Promise<void>;
  askAboutMeeting: (meetingId: string, question: string) => Promise<void>;
  searchMeetings: (query: string) => Promise<SemanticSearchResult[]>;
  addMessage: (message: ChatMessage) => void;
  updateLastMessage: (content: string, isStreaming?: boolean) => void;
  setSelectedMeetings: (meetingIds: string[]) => void;
  clearMessages: () => void;
  clearError: () => void;
}

function generateId(): string {
  return Math.random().toString(36).substring(2, 15);
}

export const useChatStore = create<ChatStore>((set, get) => ({
  // Initial state
  messages: [],
  isLoading: false,
  isStreaming: false,
  error: null,
  selectedMeetingIds: [],

  sendMessage: async (content) => {
    const { selectedMeetingIds } = get();

    // Add user message
    const userMessage: ChatMessage = {
      id: generateId(),
      role: 'user',
      content,
      timestamp: Date.now(),
    };
    set((state) => ({
      messages: [...state.messages, userMessage],
      isLoading: true,
      error: null,
    }));

    try {
      // If we have selected meetings, search for relevant content first
      let sources: ChatSource[] = [];
      let answer: string;

      if (selectedMeetingIds.length > 0) {
        // Ask about specific meeting(s)
        // For now, we'll use the first selected meeting
        const meetingId = selectedMeetingIds[0];
        answer = await api.askMeetingQuestion(meetingId, content);

        // Get semantic search results for sources
        const searchResults = await api.semanticSearch(content, 3, meetingId);
        sources = searchResults.map((r) => ({
          meeting_id: r.meeting_id,
          meeting_title: '', // Will be populated later if needed
          excerpt: r.text,
          start_ms: r.start_ms,
          similarity: r.similarity,
        }));
      } else {
        // General search across all meetings
        const searchResults = await api.semanticSearch(content, 5);

        if (searchResults.length > 0) {
          // Use the first meeting for context
          answer = await api.askMeetingQuestion(
            searchResults[0].meeting_id,
            content
          );
          sources = searchResults.map((r) => ({
            meeting_id: r.meeting_id,
            meeting_title: '',
            excerpt: r.text,
            start_ms: r.start_ms,
            similarity: r.similarity,
          }));
        } else {
          answer =
            "I couldn't find any relevant information in your meetings. Try asking about a specific topic that was discussed.";
        }
      }

      // Add assistant message
      const assistantMessage: ChatMessage = {
        id: generateId(),
        role: 'assistant',
        content: answer,
        timestamp: Date.now(),
        sources: sources.length > 0 ? sources : undefined,
      };

      set((state) => ({
        messages: [...state.messages, assistantMessage],
        isLoading: false,
      }));
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
      });
    }
  },

  askAboutMeeting: async (meetingId, question) => {
    // Add user message
    const userMessage: ChatMessage = {
      id: generateId(),
      role: 'user',
      content: question,
      timestamp: Date.now(),
    };
    set((state) => ({
      messages: [...state.messages, userMessage],
      isLoading: true,
      error: null,
    }));

    try {
      const answer = await api.askMeetingQuestion(meetingId, question);

      // Get sources
      const searchResults = await api.semanticSearch(question, 3, meetingId);
      const sources: ChatSource[] = searchResults.map((r) => ({
        meeting_id: r.meeting_id,
        meeting_title: '',
        excerpt: r.text,
        start_ms: r.start_ms,
        similarity: r.similarity,
      }));

      const assistantMessage: ChatMessage = {
        id: generateId(),
        role: 'assistant',
        content: answer,
        timestamp: Date.now(),
        sources: sources.length > 0 ? sources : undefined,
      };

      set((state) => ({
        messages: [...state.messages, assistantMessage],
        isLoading: false,
      }));
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
      });
    }
  },

  searchMeetings: async (query) => {
    try {
      return await api.semanticSearch(query, 10);
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      return [];
    }
  },

  addMessage: (message) => {
    set((state) => ({
      messages: [...state.messages, message],
    }));
  },

  updateLastMessage: (content, isStreaming = false) => {
    set((state) => {
      const messages = [...state.messages];
      const lastIndex = messages.length - 1;
      if (lastIndex >= 0 && messages[lastIndex].role === 'assistant') {
        messages[lastIndex] = {
          ...messages[lastIndex],
          content,
          isStreaming,
        };
      }
      return { messages, isStreaming };
    });
  },

  setSelectedMeetings: (meetingIds) => {
    set({ selectedMeetingIds: meetingIds });
  },

  clearMessages: () => {
    set({ messages: [], error: null });
  },

  clearError: () => {
    set({ error: null });
  },
}));
