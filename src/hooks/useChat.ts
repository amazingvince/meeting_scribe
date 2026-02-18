/**
 * Chat hook
 * Provides chat interface and RAG interactions
 */

import { useCallback } from 'react';
import { useChatStore } from '../stores';

export function useChat() {
  const store = useChatStore();

  const sendMessage = useCallback(
    async (content: string) => {
      if (!content.trim()) return;
      if (store.streamingEnabled) {
        await store.sendMessageWithStreaming(content);
      } else {
        await store.sendMessage(content);
      }
    },
    [store]
  );

  const askAboutMeeting = useCallback(
    async (meetingId: string, question: string) => {
      if (!question.trim()) return;
      await store.askAboutMeeting(meetingId, question);
    },
    [store]
  );

  const searchMeetings = useCallback(
    async (query: string) => {
      return await store.searchMeetings(query);
    },
    [store]
  );

  const selectMeetings = useCallback(
    (meetingIds: string[]) => {
      store.setSelectedMeetings(meetingIds);
    },
    [store]
  );

  return {
    // State
    messages: store.messages,
    isLoading: store.isLoading,
    isStreaming: store.isStreaming,
    error: store.error,
    selectedMeetingIds: store.selectedMeetingIds,
    streamingEnabled: store.streamingEnabled,

    // Actions
    sendMessage,
    askAboutMeeting,
    searchMeetings,
    selectMeetings,
    setStreamingEnabled: store.setStreamingEnabled,
    clearMessages: store.clearMessages,
    clearError: store.clearError,
  };
}
