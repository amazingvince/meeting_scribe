import { useEffect, useMemo, useRef } from 'react';
import { useSettingsStore, useToastStore } from '../stores';
import { postProcessingManager, type PostProcessingNotifications } from '../lib/postProcessingManager';
import { useTauriEvent } from './useTauriEvent';
import type { MeetingProcessingFinishedEvent } from '../types';
import * as api from '../lib/tauri';

export function usePostProcessingCoordinator(): void {
  const llmReady = useSettingsStore((state) => state.llmReady);
  const embeddingReady = useSettingsStore((state) => state.embeddingReady);
  const autoEmbedTranscripts = useSettingsStore((state) => state.autoEmbedTranscripts);

  const toastWarning = useToastStore((state) => state.warning);
  const toastSuccess = useToastStore((state) => state.success);
  const deferredWarningCooldownRef = useRef<{ summary: number; embedding: number }>({
    summary: 0,
    embedding: 0,
  });
  const vectorRepairWarningCooldownRef = useRef(0);
  const vectorRepairInFlightRef = useRef(false);

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

  useEffect(() => {
    if (vectorRepairInFlightRef.current) {
      return;
    }

    let cancelled = false;
    vectorRepairInFlightRef.current = true;

    void (async () => {
      try {
        const result = await api.repairVectorIndexIfNeeded();
        if (cancelled || !result.needed) {
          return;
        }

        if (result.completed) {
          toastSuccess(
            'Search index repaired',
            `Re-indexed ${result.processed} meeting${result.processed === 1 ? '' : 's'}.`
          );
          return;
        }

        const now = Date.now();
        if (now - vectorRepairWarningCooldownRef.current < 60000) {
          return;
        }
        vectorRepairWarningCooldownRef.current = now;
        toastWarning('Search index rebuild pending', result.message);
      } catch (error) {
        if (cancelled) {
          return;
        }
        const now = Date.now();
        if (now - vectorRepairWarningCooldownRef.current < 60000) {
          return;
        }
        vectorRepairWarningCooldownRef.current = now;
        toastWarning(
          'Search index repair failed',
          error instanceof Error ? error.message : String(error)
        );
      } finally {
        vectorRepairInFlightRef.current = false;
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [embeddingReady, toastSuccess, toastWarning]);
}
