import { useEffect, useMemo, useRef } from 'react';
import { useSettingsStore, useToastStore } from '../stores';
import { postProcessingManager, type PostProcessingNotifications } from '../lib/postProcessingManager';
import { useTauriEvent } from './useTauriEvent';
import type { MeetingProcessingFinishedEvent } from '../types';

export function usePostProcessingCoordinator(): void {
  const llmReady = useSettingsStore((state) => state.llmReady);
  const embeddingReady = useSettingsStore((state) => state.embeddingReady);
  const autoEmbedTranscripts = useSettingsStore((state) => state.autoEmbedTranscripts);

  const toastWarning = useToastStore((state) => state.warning);
  const deferredWarningCooldownRef = useRef<{ summary: number; embedding: number }>({
    summary: 0,
    embedding: 0,
  });

  const notifications = useMemo<PostProcessingNotifications>(
    () => ({
      onSummaryStarted: () => {},
      onSummaryDeferred: () => {
        const now = Date.now();
        if (now - deferredWarningCooldownRef.current.summary < 30000) {
          return;
        }
        deferredWarningCooldownRef.current.summary = now;
        toastWarning(
          'Summary pending',
          'Language model is not available yet. Summary will start automatically once ready.'
        );
      },
      onSummaryFailed: (_meetingId, error) => {
        toastWarning('Summary generation failed to start', error);
      },
      onEmbeddingDeferred: () => {
        const now = Date.now();
        if (now - deferredWarningCooldownRef.current.embedding < 30000) {
          return;
        }
        deferredWarningCooldownRef.current.embedding = now;
        toastWarning(
          'Indexing pending',
          'Embedding model is not available yet. Indexing will resume automatically once ready.'
        );
      },
      onEmbeddingCompleted: () => {},
      onEmbeddingFailed: (_meetingId, error) => {
        toastWarning('Indexing failed', error);
      },
    }),
    [toastWarning]
  );

  useTauriEvent<MeetingProcessingFinishedEvent>('meeting-processing-finished', (event) => {
    if (!event.success) {
      return;
    }

    postProcessingManager.enqueueMeeting(event.meeting_id);
    void postProcessingManager.processPending(notifications);
  });

  useEffect(() => {
    void postProcessingManager.processPending(notifications);
  }, [llmReady, embeddingReady, autoEmbedTranscripts, notifications]);
}
