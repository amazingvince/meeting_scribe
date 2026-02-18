/**
 * Chat state store
 * Manages chat messages, streaming state, and RAG interactions
 */

import { create } from 'zustand';
import type { ChatMessage, ChatSource, SemanticSearchResult } from '../types';
import type {
  ChatHistoryMessage,
  ChatTokenEvent,
  RetrievedContextChunk,
} from '../lib/tauri';
import * as api from '../lib/tauri';
import { modelManager } from '../lib/modelManager';

/** Maximum number of history messages to include in context */
const MAX_HISTORY_MESSAGES = 6;
const MAX_RETRIEVAL_CONTEXT_CHUNKS = 10;
const MEETING_SCOPED_RETRIEVAL_LIMIT = 6;
const GLOBAL_RETRIEVAL_LIMIT = 12;

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

function mapSearchResultsToSources(results: SemanticSearchResult[]): ChatSource[] {
  return results.map((r) => ({
    meeting_id: r.meeting_id,
    meeting_title: r.meeting_title,
    excerpt: r.text,
    start_ms: r.start_ms,
    end_ms: r.end_ms,
    similarity: r.similarity,
  }));
}

function normalizeTextForKey(text: string): string {
  return text
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .join(' ');
}

function retrievalResultKey(result: SemanticSearchResult): string {
  const chunkKey =
    result.chunk_index !== null
      ? `chunk:${result.chunk_index}`
      : `time:${result.start_ms ?? -1}:${result.end_ms ?? -1}:${normalizeTextForKey(result.text)}`;
  return `${result.meeting_id}::${result.chunk_type}::${chunkKey}`;
}

function rankRetrievalHit(hit: SemanticSearchResult, rankIndex: number): number {
  const rankScore = 1 / (rankIndex + 1);
  const similarityScore = Number.isFinite(hit.similarity) ? Math.max(0, hit.similarity) : 0;
  const lexicalBoost = hit.chunk_type === 'fts' ? 0.06 : 0;
  return rankScore + similarityScore * 0.35 + lexicalBoost;
}

function rankAndDedupeResults(
  resultLists: SemanticSearchResult[][]
): SemanticSearchResult[] {
  const bestByKey = new Map<
    string,
    { hit: SemanticSearchResult; score: number }
  >();

  for (const list of resultLists) {
    for (const [rankIndex, hit] of list.entries()) {
      const key = retrievalResultKey(hit);
      const score = rankRetrievalHit(hit, rankIndex);
      const existing = bestByKey.get(key);
      if (!existing || score > existing.score) {
        bestByKey.set(key, { hit, score });
      }
    }
  }

  return [...bestByKey.values()]
    .sort((a, b) => b.score - a.score)
    .map((entry) => entry.hit);
}

function toRetrievedContextChunks(
  results: SemanticSearchResult[]
): RetrievedContextChunk[] {
  return results.map((result) => ({
    meeting_id: result.meeting_id,
    meeting_title: result.meeting_title,
    text: result.text,
    start_ms: result.start_ms ?? null,
    end_ms: result.end_ms ?? null,
    similarity: result.similarity,
  }));
}

async function searchInMeetings(
  query: string,
  meetingIds: string[],
  perMeetingLimit: number
): Promise<SemanticSearchResult[]> {
  const scopedMeetingIds = meetingIds.slice(0, 12);
  const resultLists = await Promise.all(
    scopedMeetingIds.map((meetingId) =>
      api.hybridSearch(query, perMeetingLimit, meetingId)
    )
  );
  return rankAndDedupeResults(resultLists);
}

async function searchAcrossMeetings(
  query: string,
  limit: number
): Promise<SemanticSearchResult[]> {
  const results = await api.hybridSearch(query, limit);
  return rankAndDedupeResults([results]);
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
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  return `${Math.random().toString(36).slice(2)}-${Date.now().toString(36)}`;
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
      const embeddingReady = await modelManager.ensureEmbeddingReady();
      if (!embeddingReady) {
        throw new Error(
          'Embedding model is not ready. Download and load it in Settings to use chat.'
        );
      }
      const llmReady = await modelManager.ensureLlmReady();
      if (!llmReady) {
        throw new Error(
          'Language model is not ready. Download and load a model in Settings to use chat.'
        );
      }

      // Retrieve relevant chunks first, then answer strictly from those chunks.
      let sources: ChatSource[] = [];
      let answer: string;
      let retrieved: SemanticSearchResult[] = [];

      if (selectedMeetingIds.length > 0) {
        retrieved = await searchInMeetings(
          content,
          selectedMeetingIds,
          MEETING_SCOPED_RETRIEVAL_LIMIT
        );
      } else {
        retrieved = await searchAcrossMeetings(content, GLOBAL_RETRIEVAL_LIMIT);
      }

      if (retrieved.length > 0) {
        const topChunks = retrieved.slice(0, MAX_RETRIEVAL_CONTEXT_CHUNKS);
        answer = await api.answerWithRetrieval(
          content,
          toRetrievedContextChunks(topChunks),
          history
        );
        sources = mapSearchResultsToSources(topChunks);
      } else {
        answer =
          "I couldn't find any relevant information in your meetings. Try asking about a specific topic that was discussed.";
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

    const streamId = generateId();
    let streamedContent = '';
    let unlisten: (() => void) | null = null;

    const stopListening = () => {
      if (!unlisten) return;
      try {
        unlisten();
      } finally {
        unlisten = null;
      }
    };

    try {
      const embeddingReady = await modelManager.ensureEmbeddingReady();
      if (!embeddingReady) {
        throw new Error(
          'Embedding model is not ready. Download and load it in Settings to use chat.'
        );
      }

      // Set up token listener
      unlisten = await api.onChatToken((event: ChatTokenEvent) => {
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
          stopListening();
          return;
        }

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
      });

      // Determine which meeting to query
      let meetingId: string | null = null;
      if (selectedMeetingIds.length > 0) {
        const selectedSearchResults = await searchInMeetings(
          content,
          selectedMeetingIds,
          2
        );
        meetingId = selectedSearchResults[0]?.meeting_id ?? selectedMeetingIds[0];
      } else {
        // Search for relevant meeting
        const searchResults = await api.semanticSearch(content, 1);
        if (searchResults.length > 0) {
          meetingId = searchResults[0].meeting_id;
        }
      }

      if (!meetingId) {
        set((state) => {
          const msgs = [...state.messages];
          const lastIdx = msgs.length - 1;
          if (lastIdx >= 0 && msgs[lastIdx].role === 'assistant') {
            msgs[lastIdx] = {
              ...msgs[lastIdx],
              content: 'No meetings found to search.',
              isStreaming: false,
            };
          }
          return {
            messages: msgs,
            error: 'No meetings found to search',
            isLoading: false,
            isStreaming: false,
          };
        });
        stopListening();
        return;
      }

      const llmReady = await modelManager.ensureLlmReady();
      if (!llmReady) {
        throw new Error(
          'Language model is not ready. Download and load a model in Settings to use chat.'
        );
      }

      // Start streaming (final completion handled by the `done` event)
      await api.streamMeetingQuestion(streamId, meetingId, content, history);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      stopListening();
      set((state) => {
        const msgs = [...state.messages];
        const lastIdx = msgs.length - 1;
        if (lastIdx >= 0 && msgs[lastIdx].role === 'assistant') {
          msgs[lastIdx] = {
            ...msgs[lastIdx],
            content: `Error: ${message}`,
            isStreaming: false,
          };
        }
        return {
          messages: msgs,
          error: message,
          isLoading: false,
          isStreaming: false,
        };
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
      const llmReady = await modelManager.ensureLlmReady();
      if (!llmReady) {
        throw new Error(
          'Language model is not ready. Download and load a model in Settings to ask questions.'
        );
      }

      const retrieved = await api.hybridSearch(
        question,
        MAX_RETRIEVAL_CONTEXT_CHUNKS,
        meetingId
      );
      const topChunks = rankAndDedupeResults([retrieved]).slice(
        0,
        MAX_RETRIEVAL_CONTEXT_CHUNKS
      );

      const answer =
        topChunks.length > 0
          ? await api.answerWithRetrieval(
              question,
              toRetrievedContextChunks(topChunks),
              history
            )
          : await api.askMeetingQuestion(meetingId, question, history);

      // Get sources
      let sources: ChatSource[] = [];
      if (topChunks.length > 0) {
        sources = mapSearchResultsToSources(topChunks);
      }

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
      const embeddingReady = await modelManager.ensureEmbeddingReady();
      if (!embeddingReady) {
        set({
          error:
            'Embedding model is not ready. Download and load it in Settings to search meetings.',
        });
        return [];
      }
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
