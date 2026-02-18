/**
 * Models hook
 * Provides model status and download progress
 */

import { useEffect, useRef } from 'react';
import { useSettingsStore, useToastStore } from '../stores';
import { useTauriEvent } from './useTauriEvent';
import type {
  TranscriptionBackend,
  LlmDownloadProgress,
  EmbeddingDownloadProgress,
} from '../types';
import type { DownloadProgressEvent, BatchEmbedProgress } from '../lib/tauri';

const transcriptionModelIdMap: Record<string, TranscriptionBackend> = {
  'parakeet-tdt-0.6b-v3-int8': 'Parakeet',
  'whisper-medium-q4_1': 'Whisper',
  'moonshine-tiny': 'Moonshine',
};

function asTranscriptionBackend(
  value: string | null | undefined
): TranscriptionBackend | undefined {
  if (value === 'Parakeet' || value === 'Whisper' || value === 'Moonshine') {
    return value;
  }
  return undefined;
}

export function useModels() {
  const store = useSettingsStore();
  const toast = useToastStore();
  const {
    setDownloadProgress,
    refreshModelStatus,
    setBatchEmbedProgress,
    refreshUnembeddedCount,
  } = store;

  // Track if we've shown the completion toast to avoid duplicates
  const completedModelsRef = useRef<Set<string>>(new Set());

  // Subscribe to download progress events
  useTauriEvent<DownloadProgressEvent>('model-download-progress', (data) => {
    const activeDownload = useSettingsStore.getState().downloadingModel;
    const modelKey =
      transcriptionModelIdMap[data.model_id] ??
      asTranscriptionBackend(activeDownload);
    const toastKey = modelKey ?? data.model_id;

    setDownloadProgress({
      progress: data.percent,
      modelKey,
      sourceModelId: data.model_id,
      stage: data.stage,
      message: data.message,
      downloadedBytes: data.downloaded_bytes ?? null,
      totalBytes: data.total_bytes ?? null,
      speedBps: data.speed_bps ?? null,
    });

    // Show completion toast when reaching 100%
    if (data.percent >= 100 && !completedModelsRef.current.has(toastKey)) {
      completedModelsRef.current.add(toastKey);
      toast.success(`${modelKey ?? data.model_id} downloaded successfully`);
      // Clear after a delay to allow re-download
      setTimeout(() => completedModelsRef.current.delete(toastKey), 5000);
    }
  });

  useTauriEvent<LlmDownloadProgress>('llm-download-progress', (data) => {
    setDownloadProgress({
      progress: data.percent,
      modelKey: data.model,
      sourceModelId: data.model,
      stage: 'Downloading',
      message: `Downloading ${data.model}...`,
      downloadedBytes: data.downloaded_bytes,
      totalBytes: data.total_bytes,
      speedBps: data.speed_bps ?? null,
    });

    // Show completion toast when reaching 100%
    if (data.percent >= 100 && !completedModelsRef.current.has(data.model)) {
      completedModelsRef.current.add(data.model);
      toast.success(`${data.model} downloaded successfully`);
      setTimeout(() => completedModelsRef.current.delete(data.model), 5000);
    }
  });

  useTauriEvent<EmbeddingDownloadProgress>(
    'embedding-download-progress',
    (data) => {
      const toastKey = 'embedding';
      setDownloadProgress({
        progress: data.percent,
        modelKey: 'embedding',
        sourceModelId: data.model_id,
        stage: data.status,
        message: `Downloading ${data.file}...`,
        downloadedBytes: data.downloaded,
        totalBytes: data.total,
        speedBps: null,
        file: data.file,
      });

      if (data.percent >= 100 && !completedModelsRef.current.has(toastKey)) {
        completedModelsRef.current.add(toastKey);
        toast.success('Embedding model downloaded successfully');
        setTimeout(() => completedModelsRef.current.delete(toastKey), 5000);
      }
    }
  );

  // Listen for batch embed progress
  useTauriEvent<BatchEmbedProgress>('batch-embed-progress', (data) => {
    if (data.status === 'complete') {
      setBatchEmbedProgress(null);
      toast.success(`Embedded ${data.total} meeting(s) successfully`);
      refreshUnembeddedCount();
    } else {
      setBatchEmbedProgress({
        current: data.current,
        total: data.total,
        currentMeeting: data.current_meeting,
      });
    }
  });

  // Refresh model status and unembedded count on mount
  useEffect(() => {
    refreshModelStatus();
    refreshUnembeddedCount();
  }, [refreshModelStatus, refreshUnembeddedCount]);

  return {
    // Model status
    transcriptionDownloaded: store.transcriptionDownloaded,
    transcriptionReady: store.transcriptionReady,
    embeddingDownloaded: store.embeddingDownloaded,
    embeddingReady: store.embeddingReady,
    llmReady: store.llmReady,
    llmStatus: store.llmStatus,
    llmModels: store.llmModels,
    embeddingInfo: store.embeddingInfo,

    // Loading states
    isLoadingModels: store.isLoadingModels,
    isDownloading: store.isDownloading,
    isLoadingTranscription: store.isLoadingTranscription,
    isLoadingEmbedding: store.isLoadingEmbedding,
    isLoadingLlm: store.isLoadingLlm,
    downloadProgress: store.downloadProgress,
    downloadingModel: store.downloadingModel,
    downloadStage: store.downloadStage,
    downloadMessage: store.downloadMessage,
    downloadedBytes: store.downloadedBytes,
    downloadTotalBytes: store.downloadTotalBytes,
    downloadSpeedBps: store.downloadSpeedBps,
    downloadFile: store.downloadFile,
    downloadSourceModelId: store.downloadSourceModelId,
    error: store.error,
    errorModel: store.errorModel,

    // Actions
    refreshModelStatus: store.refreshModelStatus,
    initializeTranscription: store.initializeTranscription,
    initializeEmbedding: store.initializeEmbedding,
    initializeLlm: store.initializeLlm,
    downloadTranscriptionModel: store.downloadTranscriptionModel,
    downloadEmbeddingModel: store.downloadEmbeddingModel,
    downloadLlmModel: store.downloadLlmModel,
    deleteTranscriptionModel: store.deleteTranscriptionModel,
    deleteEmbeddingModel: store.deleteEmbeddingModel,
    deleteLlmModel: store.deleteLlmModel,
    clearError: store.clearError,

    // Batch embedding
    unembeddedCount: store.unembeddedCount,
    isBatchEmbedding: store.isBatchEmbedding,
    batchEmbedProgress: store.batchEmbedProgress,
    batchEmbedMeetings: store.batchEmbedMeetings,
    refreshUnembeddedCount: store.refreshUnembeddedCount,
  };
}
