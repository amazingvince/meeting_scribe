/**
 * Chat-related TypeScript types
 * For RAG chat interface
 */

/** Chat message role */
export type ChatRole = 'user' | 'assistant' | 'system';

/** Chat message */
export interface ChatMessage {
  /** Unique message ID */
  id: string;
  /** Message role */
  role: ChatRole;
  /** Message content */
  content: string;
  /** Timestamp when message was created */
  timestamp: number;
  /** Sources used to generate the response (for assistant messages) */
  sources?: ChatSource[];
  /** Whether the message is still being streamed */
  isStreaming?: boolean;
}

/** Source citation for RAG responses */
export interface ChatSource {
  /** Meeting ID this source is from */
  meeting_id: string;
  /** Meeting title for display */
  meeting_title: string;
  /** Relevant text excerpt */
  excerpt: string;
  /** Timestamp in the meeting (ms) */
  start_ms: number | null;
  /** Similarity score (0-1) */
  similarity: number;
}

/** Chat session */
export interface ChatSession {
  /** Session ID */
  id: string;
  /** Session title */
  title: string;
  /** Messages in this session */
  messages: ChatMessage[];
  /** Meeting IDs included in this chat context */
  meeting_ids: string[];
  /** Created timestamp */
  created_at: number;
  /** Last updated timestamp */
  updated_at: number;
}

/** Semantic search result from embedding search */
export interface SemanticSearchResult {
  id: string;
  meeting_id: string;
  meeting_title: string;
  chunk_type: string;
  text: string;
  start_ms: number | null;
  similarity: number;
}

/** Chat input state */
export interface ChatInputState {
  /** Current input text */
  text: string;
  /** Whether a response is being generated */
  isLoading: boolean;
  /** Whether response is streaming */
  isStreaming: boolean;
}

/** Suggested chat prompts */
export interface ChatSuggestion {
  /** Display label */
  label: string;
  /** Full prompt to send */
  prompt: string;
  /** Category for grouping */
  category: 'summary' | 'action' | 'search' | 'general';
}

/** Default chat suggestions */
export const defaultChatSuggestions: ChatSuggestion[] = [
  {
    label: 'Summarize recent meetings',
    prompt: 'Give me a summary of my recent meetings',
    category: 'summary',
  },
  {
    label: 'List action items',
    prompt: 'What action items came out of my recent meetings?',
    category: 'action',
  },
  {
    label: 'Find discussions about...',
    prompt: 'What was discussed about ',
    category: 'search',
  },
  {
    label: 'Key decisions made',
    prompt: 'What key decisions were made in recent meetings?',
    category: 'general',
  },
];
