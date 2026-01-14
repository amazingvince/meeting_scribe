/**
 * Chat state store
 * Manages chat messages, streaming state, and RAG interactions
 */

import { create } from 'zustand';
import type { ChatMessage, ChatSource, SemanticSearchResult } from '../types';
import type { ChatHistoryMessage, ChatTokenEvent } from '../lib/tauri';
import * as api from '../lib/tauri';

/** Maximum number of history messages to include in context */
const MAX_HISTORY_MESSAGES = 6;

/** Convert chat messages to history format for LLM */
function getHistoryForLlm(messages: ChatMessage[]): ChatHistoryMessage[] {
  // Get the last N messages (excluding the current one being processed)
  const recentMessages = messages.slice(-MAX_HISTORY_MESSAGES);
  return recentMessages
    .filter((m) => m.role === 'user' || m.role === 'assistant')
    .map((m) => ({
      role: m.role as 'user' | 'assistant',
      content: m.content,
    }));
}

interface ChatStore {
  // State
  messages: ChatMessage[];
  isLoading: boolean;
  isStreaming: boolean;
  error: string | null;
  selectedMeetingIds: string[];
  streamingEnabled: boolean;

  // Actions
  sendMessage: (content: string) => Promise<void>;
  sendMessageWithStreaming: (content: string) => Promise<void>;
  askAboutMeeting: (meetingId: string, question: string) => Promise<void>;
  searchMeetings: (query: string) => Promise<SemanticSearchResult[]>;
  addMessage: (message: ChatMessage) => void;
  updateLastMessage: (content: string, isStreaming?: boolean) => void;
  setSelectedMeetings: (meetingIds: string[]) => void;
  setStreamingEnabled: (enabled: boolean) => void;
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
  streamingEnabled: false, // Disabled by default for stability

  sendMessage: async (content) => {
    const { selectedMeetingIds, messages } = get();

    // Get conversation history before adding new message
    const history = getHistoryForLlm(messages);

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
        answer = await api.askMeetingQuestion(meetingId, content, history);

        // Get semantic search results for sources
        const searchResults = await api.semanticSearch(content, 3, meetingId);
        sources = searchResults.map((r) => ({
          meeting_id: r.meeting_id,
          meeting_title: r.meeting_title,
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
            content,
            history
          );
          sources = searchResults.map((r) => ({
            meeting_id: r.meeting_id,
            meeting_title: r.meeting_title,
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

  sendMessageWithStreaming: async (content) => {
    const { selectedMeetingIds, messages } = get();

    // Get conversation history before adding new message
    const history = getHistoryForLlm(messages);

    // Add user message
    const userMessage: ChatMessage = {
      id: generateId(),
      role: 'user',
      content,
      timestamp: Date.now(),
    };

    // Add empty assistant message for streaming
    const assistantMessage: ChatMessage = {
      id: generateId(),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
      isStreaming: true,
    };

    set((state) => ({
      messages: [...state.messages, userMessage, assistantMessage],
      isLoading: true,
      isStreaming: true,
      error: null,
    }));

    try {
      const streamId = generateId();
      let streamedContent = '';

      // Set up token listener
      const unlisten = await api.onChatToken((event: ChatTokenEvent) => {
        if (event.stream_id !== streamId) return;

        if (event.done) {
          // Streaming complete
          set((state) => {
            const msgs = [...state.messages];
            const lastIdx = msgs.length - 1;
            if (lastIdx >= 0 && msgs[lastIdx].role === 'assistant') {
              msgs[lastIdx] = {
                ...msgs[lastIdx],
                content: streamedContent.trim(),
                isStreaming: false,
              };
            }
            return { messages: msgs, isLoading: false, isStreaming: false };
          });
          unlisten();
        } else {
          // Append token
          streamedContent += event.token;
          set((state) => {
            const msgs = [...state.messages];
            const lastIdx = msgs.length - 1;
            if (lastIdx >= 0 && msgs[lastIdx].role === 'assistant') {
              msgs[lastIdx] = {
                ...msgs[lastIdx],
                content: streamedContent,
              };
            }
            return { messages: msgs };
          });
        }
      });

      // Determine which meeting to query
      let meetingId: string | null = null;
      if (selectedMeetingIds.length > 0) {
        meetingId = selectedMeetingIds[0];
      } else {
        // Search for relevant meeting
        const searchResults = await api.semanticSearch(content, 1);
        if (searchResults.length > 0) {
          meetingId = searchResults[0].meeting_id;
        }
      }

      if (!meetingId) {
        set({
          error: "No meetings found to search",
          isLoading: false,
          isStreaming: false,
        });
        unlisten();
        return;
      }

      // Start streaming
      await api.streamMeetingQuestion(streamId, meetingId, content, history);
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
        isStreaming: false,
      });
    }
  },

  askAboutMeeting: async (meetingId, question) => {
    const { messages } = get();

    // Get conversation history before adding new message
    const history = getHistoryForLlm(messages);

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
      const answer = await api.askMeetingQuestion(meetingId, question, history);

      // Get sources
      const searchResults = await api.semanticSearch(question, 3, meetingId);
      const sources: ChatSource[] = searchResults.map((r) => ({
        meeting_id: r.meeting_id,
        meeting_title: r.meeting_title,
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

  setStreamingEnabled: (enabled) => {
    set({ streamingEnabled: enabled });
  },

  clearMessages: () => {
    set({ messages: [], error: null });
  },

  clearError: () => {
    set({ error: null });
  },
}));
